//! Adaptador de Codex (CLI de OpenAI). Usa `codex exec --json` con la sesión de ChatGPT del propio CLI,
//! sin clave de API. En este modo Codex no puede preguntar permisos: se limita con el sandbox.
use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::events::ProviderEvent;
use crate::provider::{fold_history, fold_messages, Provider, ProviderError, TurnRequest};

/// Un modelo de Codex tal como lo anuncia la cuenta del usuario.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct CodexModel {
    pub id: String,
    pub label: String,
    pub description: String,
    /// Niveles de esfuerzo que admite, con su explicación (en inglés, tal como los da Codex).
    pub efforts: Vec<(String, String)>,
    pub default_effort: Option<String>,
}

/// Lo que Codex sabe de la cuenta: sus modelos visibles y el modelo y esfuerzo que el usuario dejó por defecto.
#[derive(Debug, Clone, serde::Serialize, Default, PartialEq)]
pub struct CodexCatalog {
    pub models: Vec<CodexModel>,
    pub default_model: Option<String>,
    pub default_effort: Option<String>,
}

fn codex_home() -> Option<PathBuf> {
    match std::env::var_os("CODEX_HOME") {
        Some(h) if !h.is_empty() => Some(PathBuf::from(h)),
        _ => Some(PathBuf::from(std::env::var_os("HOME")?).join(".codex")),
    }
}

/// Lee `models_cache.json` (lo mantiene Codex al día con la cuenta) y `config.toml` (valores por defecto).
pub fn catalog() -> CodexCatalog {
    let Some(home) = codex_home() else { return CodexCatalog::default() };
    let models = std::fs::read_to_string(home.join("models_cache.json")).ok().map(|t| parse_models(&t)).unwrap_or_default();
    let config = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let (default_model, default_effort) = parse_defaults(&config);
    CodexCatalog { models, default_model, default_effort }
}

pub fn parse_models(text: &str) -> Vec<CodexModel> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let mut list: Vec<(i64, CodexModel)> = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"].as_str().unwrap_or("list") == "list")
        .filter_map(|m| {
            let id = m["slug"].as_str()?.to_string();
            let efforts = m["supported_reasoning_levels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| Some((e["effort"].as_str()?.to_string(), e["description"].as_str().unwrap_or("").to_string())))
                .collect();
            Some((
                m["priority"].as_i64().unwrap_or(i64::MAX),
                CodexModel {
                    label: m["display_name"].as_str().unwrap_or(&id).to_string(),
                    description: m["description"].as_str().unwrap_or("").to_string(),
                    default_effort: m["default_reasoning_level"].as_str().map(String::from),
                    efforts,
                    id,
                },
            ))
        })
        .collect();
    list.sort_by_key(|(p, _)| *p);
    list.into_iter().map(|(_, m)| m).collect()
}

/// `model = "..."` y `model_reasoning_effort = "..."` del nivel superior de `config.toml` (antes de la primera sección).
pub fn parse_defaults(config: &str) -> (Option<String>, Option<String>) {
    let (mut model, mut effort) = (None, None);
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim().trim_matches('"').to_string();
        match k.trim() {
            "model" if !v.is_empty() => model = Some(v),
            "model_reasoning_effort" if !v.is_empty() => effort = Some(v),
            _ => {}
        }
    }
    (model, effort)
}

/// Dónde está el ejecutable: `COMODEL_CODEX`, el `codex` del PATH o el que trae la app de ChatGPT.
pub fn find_binary() -> Option<String> {
    if let Ok(p) = std::env::var("COMODEL_CODEX") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    let on_path = std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join("codex").is_file()));
    if on_path {
        return Some("codex".into());
    }
    let bundled = PathBuf::from(std::env::var_os("HOME")?).join(".local/lib/chatgpt/resources/codex");
    bundled.is_file().then(|| bundled.display().to_string())
}

pub struct Codex {
    id: String,
    binary: String,
    /// Sin carpeta de trabajo (chat sin proyecto) Codex solo lee y conversa.
    read_only: bool,
}

impl Codex {
    pub fn new(id: impl Into<String>, binary: impl Into<String>, read_only: bool) -> Self {
        Self { id: id.into(), binary: binary.into(), read_only }
    }

    fn args(&self, req: &TurnRequest) -> Vec<String> {
        let plan = matches!(req.mode.as_deref(), Some("plan"));
        let sandbox = if self.read_only || plan { "read-only" } else { "workspace-write" };
        // Cada chat sigue en la misma sesión de Codex mientras se pueda. `exec resume` no acepta `-s` ni `-C`:
        // el sandbox va por configuración y la carpeta es la del proceso.
        let mut a: Vec<String> = if req.resume.is_some() {
            let mut a: Vec<String> = ["exec", "resume", "--json", "--skip-git-repo-check", "--ignore-user-config", "-c"].iter().map(|s| s.to_string()).collect();
            a.push(format!("sandbox_mode=\"{sandbox}\""));
            a
        } else {
            let mut a: Vec<String> = ["exec", "--json", "--skip-git-repo-check", "--ignore-user-config", "-s", sandbox].iter().map(|s| s.to_string()).collect();
            if let Some(cwd) = &req.cwd {
                a.push("-C".into());
                a.push(cwd.display().to_string());
            }
            a
        };
        // Solo llega un modelo si Codex es la IA elegida en el chat; uno de Claude u OpenRouter nunca vale aquí.
        if let Some(m) = req.model.as_deref().filter(|m| !m.starts_with("claude") && !m.contains('/')) {
            a.push("-m".into());
            a.push(m.into());
        }
        if let Some(e) = req.effort.as_deref().filter(|e| matches!(*e, "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra")) {
            a.push("-c".into());
            a.push(format!("model_reasoning_effort=\"{e}\""));
        }
        if let Some(r) = &req.resume {
            a.push(r.session_id.clone());
        }
        a.push("-".into()); // el mensaje llega por stdin
        a
    }
}

/// Un límite de uso o de peticiones, no un error cualquiera.
fn is_limit(msg: &str) -> bool {
    let l = msg.to_lowercase();
    l.contains("usage limit") || l.contains("rate limit") || l.contains("rate_limit") || l.contains("quota") || l.contains("429") || l.contains("limit reached") || l.contains("hit your")
}

fn failure(msg: &str) -> ProviderEvent {
    if is_limit(msg) {
        ProviderEvent::LimitReached { reason: msg.trim().to_string(), resets_at: None }
    } else {
        ProviderEvent::Error(msg.trim().to_string())
    }
}

/// `/usr/bin/bash -lc "ls"` se muestra como `ls`.
fn clean_command(cmd: &str) -> String {
    for prefix in ["/usr/bin/bash -lc ", "/bin/bash -lc ", "bash -lc ", "/bin/sh -c ", "sh -c "] {
        if let Some(rest) = cmd.strip_prefix(prefix) {
            let rest = rest.trim();
            let unquoted = rest.strip_prefix('"').and_then(|r| r.strip_suffix('"')).or_else(|| rest.strip_prefix('\'').and_then(|r| r.strip_suffix('\''))).unwrap_or(rest);
            return unquoted.replace("\\\"", "\"");
        }
    }
    cmd.to_string()
}

/// Traduce una línea de `codex exec --json` a eventos normalizados. `first_text` evita juntar mensajes sin separación.
pub fn parse_line(line: &str, first_text: &mut bool) -> Vec<ProviderEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    let mut out = Vec::new();
    match v["type"].as_str() {
        Some("thread.started") => out.push(ProviderEvent::Started { session_id: v["thread_id"].as_str().map(String::from), model: None, mode: None }),
        Some("item.started") if v["item"]["type"] == "command_execution" => {
            let cmd = clean_command(v["item"]["command"].as_str().unwrap_or(""));
            out.push(ProviderEvent::ToolUse { name: "Bash".into(), input: serde_json::json!({ "command": cmd }) });
        }
        Some("item.completed") => {
            let item = &v["item"];
            match item["type"].as_str() {
                Some("agent_message") => {
                    if let Some(t) = item["text"].as_str().filter(|t| !t.trim().is_empty()) {
                        let sep = if *first_text { "" } else { "\n\n" };
                        *first_text = false;
                        out.push(ProviderEvent::Text(format!("{sep}{t}")));
                    }
                }
                Some("file_change") => {
                    for c in item["changes"].as_array().into_iter().flatten() {
                        if let Some(p) = c["path"].as_str() {
                            out.push(ProviderEvent::FileTouched { path: p.to_string() });
                        }
                    }
                }
                Some("error") => {
                    if let Some(m) = item["message"].as_str() {
                        out.push(failure(m));
                    }
                }
                _ => {}
            }
        }
        Some("turn.completed") => {
            let u = &v["usage"];
            out.push(ProviderEvent::Usage {
                input_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                context_window: None,
                cost_usd: None,
            });
            out.push(ProviderEvent::Finished);
        }
        Some("turn.failed") => out.push(failure(v["error"]["message"].as_str().unwrap_or("Codex falló sin dar detalles."))),
        Some("error") => out.push(failure(v["message"].as_str().unwrap_or("Codex falló sin dar detalles."))),
        _ => {}
    }
    out
}

#[async_trait]
impl Provider for Codex {
    fn id(&self) -> &str {
        &self.id
    }

    fn resumable(&self) -> bool {
        true
    }

    async fn run_turn(&self, req: TurnRequest, tx: mpsc::Sender<ProviderEvent>) -> Result<(), ProviderError> {
        if req.resume.is_none() {
            return self.run_once(req, tx).await;
        }
        // Si la sesión ya no existe, se empieza otra con el historial del chat; el intento fallido no llega a la interfaz.
        let (itx, mut irx) = mpsc::channel::<ProviderEvent>(64);
        let lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = lost.clone();
        let out = tx.clone();
        let forward = tokio::spawn(async move {
            while let Some(ev) = irx.recv().await {
                let drop_it = match &ev {
                    ProviderEvent::Error(e) | ProviderEvent::LimitReached { reason: e, .. } if session_lost(e) => {
                        flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        true
                    }
                    ProviderEvent::Usage { .. } | ProviderEvent::Started { .. } => flag.load(std::sync::atomic::Ordering::SeqCst),
                    _ => false,
                };
                if !drop_it && out.send(ev).await.is_err() {
                    break;
                }
            }
        });
        let first = self.run_once(req.clone(), itx).await;
        let _ = forward.await;
        if lost.load(std::sync::atomic::Ordering::SeqCst) {
            return self.run_once(TurnRequest { resume: None, ..req }, tx).await;
        }
        first
    }
}

/// Codex no encontró la sesión que se le pidió continuar.
fn session_lost(error: &str) -> bool {
    let e = error.to_lowercase();
    e.contains("no session") || e.contains("session not found") || e.contains("no rollout") || e.contains("no conversation") || (e.contains("session") && e.contains("not found")) || e.contains("thread not found")
}

impl Codex {
    async fn run_once(&self, req: TurnRequest, tx: mpsc::Sender<ProviderEvent>) -> Result<(), ProviderError> {
        let mut cmd = Command::new(&self.binary);
        cmd.args(self.args(&req)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        if let Some(cwd) = &req.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn().map_err(|e| ProviderError::Spawn(format!("{}: {e}", self.binary)))?;

        let mut prompt = String::new();
        if let Some(p) = &req.preamble {
            prompt.push_str(p);
            prompt.push_str("\n\n---\n\n");
        }
        match &req.resume {
            Some(r) => prompt.push_str(&fold_messages(&r.missed, &req.prompt)),
            None => prompt.push_str(&fold_history(&req)),
        }
        let mut stdin = child.stdin.take().expect("stdin piped");
        tokio::spawn(async move {
            let _ = stdin.write_all(prompt.as_bytes()).await;
            let _ = stdin.shutdown().await; // sin cerrar stdin, Codex sigue esperando más texto
        });

        let mut stderr = child.stderr.take().expect("stderr piped");
        let stderr_task = tokio::spawn(async move {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s).await;
            s
        });

        let mut lines = BufReader::new(child.stdout.take().expect("stdout piped")).lines();
        let (mut terminal, mut first_text) = (false, true);
        while let Some(line) = lines.next_line().await? {
            for ev in parse_line(&line, &mut first_text) {
                terminal |= matches!(ev, ProviderEvent::Finished | ProviderEvent::LimitReached { .. } | ProviderEvent::Error(_));
                if tx.send(ev).await.is_err() {
                    return Ok(());
                }
            }
        }
        let status = child.wait().await?;
        let stderr = stderr_task.await.unwrap_or_default();
        if !terminal {
            // Las líneas ERROR de habilidades que no cargan no son la causa; se muestran las últimas.
            let tail: String = stderr.lines().filter(|l| !l.contains("failed to load skill")).collect::<Vec<_>>().join("\n");
            let tail: String = tail.trim().chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
            let _ = tx.send(failure(&format!("codex terminó sin resultado ({status}): {tail}"))).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Vec<ProviderEvent> {
        let mut first = true;
        lines.iter().flat_map(|l| parse_line(l, &mut first)).collect()
    }

    #[test]
    fn real_session_lines_become_events() {
        let ev = run(&[
            r#"{"type":"thread.started","thread_id":"t1"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.started","item":{"id":"i0","type":"command_execution","command":"/usr/bin/bash -lc \"printf 'a' > n.txt\"","status":"in_progress"}}"#,
            r#"{"type":"item.completed","item":{"id":"i0","type":"command_execution","command":"x","exit_code":0}}"#,
            r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"hola"}}"#,
            r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"listo"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":3}}"#,
        ]);
        assert_eq!(ev[0], ProviderEvent::Started { session_id: Some("t1".into()), model: None, mode: None });
        assert_eq!(ev[1], ProviderEvent::ToolUse { name: "Bash".into(), input: serde_json::json!({"command":"printf 'a' > n.txt"}) });
        assert_eq!(ev[2], ProviderEvent::Text("hola".into()));
        assert_eq!(ev[3], ProviderEvent::Text("\n\nlisto".into()));
        assert!(matches!(ev[4], ProviderEvent::Usage { input_tokens: 10, output_tokens: 3, .. }));
        assert_eq!(ev[5], ProviderEvent::Finished);
    }

    #[test]
    fn catalog_keeps_visible_models_in_priority_order() {
        let cache = r#"{"models":[
            {"slug":"b","display_name":"B","priority":5,"visibility":"list","default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low","description":"fast"}]},
            {"slug":"hidden","display_name":"H","priority":1,"visibility":"hide"},
            {"slug":"a","display_name":"A","priority":1,"visibility":"list","supported_reasoning_levels":[{"effort":"low"},{"effort":"ultra","description":"max"}]}]}"#;
        let m = parse_models(cache);
        assert_eq!(m.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(m[0].efforts.len(), 2);
        assert_eq!(m[1].default_effort.as_deref(), Some("medium"));
        let (model, effort) = parse_defaults("model = \"gpt-6-luna\"\nmodel_reasoning_effort = \"high\"\n[x]\nmodel = \"no\"");
        assert_eq!((model.as_deref(), effort.as_deref()), (Some("gpt-6-luna"), Some("high")));
    }

    #[test]
    fn limits_are_told_apart_from_errors() {
        assert!(matches!(run(&[r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit. Try again later."}}"#])[0], ProviderEvent::LimitReached { .. }));
        assert!(matches!(run(&[r#"{"type":"error","message":"network down"}"#])[0], ProviderEvent::Error(_)));
    }

    #[test]
    fn sandbox_follows_folder_and_mode() {
        let req = TurnRequest { mode: Some("plan".into()), model: Some("claude-haiku-5-5".into()), effort: Some("xhigh".into()), ..Default::default() };
        let a = Codex::new("codex", "codex", false).args(&req);
        assert!(a.windows(2).any(|w| w == ["-s", "read-only"]));
        assert!(!a.contains(&"-m".to_string()), "no se pasa un modelo de Claude");
        assert!(a.contains(&"model_reasoning_effort=\"xhigh\"".to_string()));
        let gpt = TurnRequest { model: Some("gpt-6.1-sol".into()), effort: Some("nope".into()), ..Default::default() };
        let a = Codex::new("codex", "codex", false).args(&gpt);
        assert!(a.windows(2).any(|w| w == ["-m", "gpt-6.1-sol"]));
        assert!(!a.iter().any(|x| x.contains("nope")));
        let a = Codex::new("codex", "codex", false).args(&TurnRequest::default());
        assert!(a.windows(2).any(|w| w == ["-s", "workspace-write"]));
        let a = Codex::new("codex", "codex", true).args(&TurnRequest::default());
        assert!(a.windows(2).any(|w| w == ["-s", "read-only"]));
    }
}
