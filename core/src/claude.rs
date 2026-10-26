//! Adaptador de Claude Code por CLI (`claude -p --output-format stream-json`).

use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::events::{AskQuestion, ProviderEvent};
use crate::interactions::Answer;
use crate::provider::{fold_history, Provider, ProviderError, TurnRequest};

const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

pub struct ClaudeCode {
    pub id: String,
    pub binary: String,
    /// Herramientas permitidas (`--tools`). `None` = las de siempre; `Some("")` = ninguna.
    pub tools: Option<String>,
}

impl ClaudeCode {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), binary: "claude".into(), tools: None }
    }

    fn args(&self, req: &TurnRequest) -> Vec<String> {
        // Sin estas banderas Claude Code carga todos los plugins, MCPs y skills del usuario
        // (en la prueba: 292k tokens por un "ok"; aislado: ~3k).
        let mut a: Vec<String> = [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            // Los permisos y las preguntas de la IA llegan por stdout y se responden por stdin.
            "--permission-prompt-tool",
            "stdio",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--setting-sources",
            "project",
            "--no-session-persistence",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if let Some(t) = &self.tools {
            a.extend(["--tools".into(), t.clone()]);
        }
        if let Some(m) = &req.model {
            a.extend(["--model".into(), m.clone()]);
        }
        if let Some(e) = &req.effort {
            a.extend(["--effort".into(), e.clone()]);
        }
        if let Some(m) = &req.mode {
            a.extend(["--permission-mode".into(), m.clone()]);
        }
        if let Some(p) = &req.preamble {
            a.extend(["--append-system-prompt".into(), p.clone()]);
        }
        a
    }
}

/// Petición de la IA a mitad de un turno: permiso para una herramienta o una pregunta al usuario.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlRequest {
    pub request_id: String,
    pub tool: String,
    pub input: Value,
    pub description: Option<String>,
    pub can_accept_edits: bool,
}

impl ControlRequest {
    pub fn is_question(&self) -> bool {
        self.tool == "AskUserQuestion"
    }

    fn questions(&self) -> Vec<AskQuestion> {
        serde_json::from_value(self.input["questions"].clone()).unwrap_or_default()
    }
}

/// Reconoce una línea `control_request` de tipo `can_use_tool`.
pub fn parse_control_request(line: &str) -> Option<ControlRequest> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v["type"] != "control_request" || v["request"]["subtype"] != "can_use_tool" {
        return None;
    }
    let r = &v["request"];
    let can_accept_edits = r["permission_suggestions"]
        .as_array()
        .is_some_and(|s| s.iter().any(|x| x["type"] == "setMode" && x["mode"] == "acceptEdits"));
    Some(ControlRequest {
        request_id: v["request_id"].as_str()?.to_string(),
        tool: r["tool_name"].as_str()?.to_string(),
        input: r["input"].clone(),
        description: r["description"].as_str().map(String::from),
        can_accept_edits,
    })
}

impl ControlRequest {
    /// Edición del archivo de memoria de CoModel (`.comodel/MEMORY.md` dentro de la carpeta de trabajo):
    /// es el propio mecanismo del producto y se permite sin molestar al usuario.
    pub fn is_memory_edit(&self, cwd: Option<&std::path::Path>) -> bool {
        if !matches!(self.tool.as_str(), "Write" | "Edit" | "MultiEdit") {
            return false;
        }
        let Some(path) = self.input["file_path"].as_str().map(std::path::Path::new) else {
            return false;
        };
        path.ends_with(".comodel/MEMORY.md") && cwd.is_none_or(|c| path.starts_with(c) || path.canonicalize().is_ok_and(|p| c.canonicalize().is_ok_and(|c| p.starts_with(c))))
    }
}

/// La línea `control_response` que contesta a `req` según lo que eligió el usuario.
/// Sin respuesta (nadie a quien preguntar) se rechaza, para no hacer nada sin permiso.
pub fn control_response(req: &ControlRequest, answer: Option<Answer>) -> String {
    let body = match answer {
        Some(Answer::Allow { always }) if !req.is_question() => {
            let mut b = serde_json::json!({"behavior": "allow", "updatedInput": req.input});
            if always && req.can_accept_edits {
                b["updatedPermissions"] = serde_json::json!([{"type": "setMode", "mode": "acceptEdits", "destination": "session"}]);
            }
            b
        }
        Some(Answer::Answers { answers }) if req.is_question() => {
            let mut input = req.input.clone();
            input["answers"] = serde_json::json!(answers);
            serde_json::json!({"behavior": "allow", "updatedInput": input})
        }
        Some(Answer::Deny { message }) => {
            serde_json::json!({"behavior": "deny", "message": message.unwrap_or_else(|| "El usuario rechazó esta acción.".into())})
        }
        _ => serde_json::json!({"behavior": "deny", "message": "No se pudo preguntar al usuario."}),
    };
    serde_json::json!({
        "type": "control_response",
        "response": {"subtype": "success", "request_id": req.request_id, "response": body}
    })
    .to_string()
}

/// Traduce una línea de `stream-json` a eventos normalizados.
pub fn parse_line(line: &str) -> Vec<ProviderEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match v["type"].as_str() {
        Some("system") if v["subtype"] == "init" => out.push(ProviderEvent::Started {
            session_id: v["session_id"].as_str().map(String::from),
            model: v["model"].as_str().map(String::from),
            mode: v["permissionMode"].as_str().map(String::from),
        }),
        Some("assistant") => {
            for block in v["message"]["content"].as_array().into_iter().flatten() {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(t) = block["text"].as_str() {
                            out.push(ProviderEvent::Text(t.to_string()));
                        }
                    }
                    Some("tool_use") => {
                        let name = block["name"].as_str().unwrap_or_default().to_string();
                        let input = block["input"].clone();
                        if EDIT_TOOLS.contains(&name.as_str()) {
                            let path = input["file_path"]
                                .as_str()
                                .or_else(|| input["notebook_path"].as_str());
                            if let Some(path) = path {
                                out.push(ProviderEvent::FileTouched { path: path.to_string() });
                            }
                        }
                        out.push(ProviderEvent::ToolUse { name, input });
                    }
                    _ => {}
                }
            }
        }
        Some("rate_limit_event") => {
            let info = &v["rate_limit_info"];
            if let Some(windows) = info["unifiedWindows"].as_object() {
                for (name, w) in windows {
                    if let Some(u) = w["utilization"].as_f64() {
                        out.push(ProviderEvent::Quota {
                            window: name.clone(),
                            used_fraction: u,
                            resets_at: w["resetsAt"].as_u64(),
                        });
                    }
                }
            }
            // Solo "allowed" fue observado en la práctica; cualquier otro estado se trata como bloqueo.
            let status = info["status"].as_str().unwrap_or("allowed");
            if status != "allowed" && status != "allowed_warning" {
                out.push(ProviderEvent::LimitReached {
                    reason: format!("{} {}", info["rateLimitType"].as_str().unwrap_or("limit"), status),
                    resets_at: info["resetsAt"].as_u64(),
                });
            }
        }
        Some("result") => {
            let u = &v["usage"];
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            let context_window = v["modelUsage"]
                .as_object()
                .and_then(|m| m.values().next())
                .and_then(|m| m["contextWindow"].as_u64());
            out.push(ProviderEvent::Usage {
                input_tokens: n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens"),
                output_tokens: n("output_tokens"),
                context_window,
                cost_usd: v["total_cost_usd"].as_f64(),
            });
            if v["is_error"].as_bool().unwrap_or(false) {
                let text = v["result"].as_str().unwrap_or("error desconocido");
                let lower = text.to_lowercase();
                let is_limit = v["api_error_status"].as_u64() == Some(429)
                    || (lower.contains("limit") && (lower.contains("usage") || lower.contains("rate") || lower.contains("reached")));
                out.push(if is_limit {
                    ProviderEvent::LimitReached { reason: text.to_string(), resets_at: None }
                } else {
                    ProviderEvent::Error(text.to_string())
                });
            } else {
                out.push(ProviderEvent::Finished);
            }
        }
        _ => {}
    }
    out
}

#[async_trait]
impl Provider for ClaudeCode {
    fn id(&self) -> &str {
        &self.id
    }

    /// Sin herramientas (chat sin carpeta) no puede abrir archivos: CoModel le pega su contenido.
    fn reads_files(&self) -> bool {
        self.tools.as_deref() != Some("")
    }

    async fn run_turn(
        &self,
        req: TurnRequest,
        tx: mpsc::Sender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let mut cmd = Command::new(&self.binary);
        cmd.args(self.args(&req))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &req.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn().map_err(|e| ProviderError::Spawn(format!("{}: {e}", self.binary)))?;

        // Todo lo que se le dice al CLI pasa por un solo escritor, para que las respuestas a permisos
        // no se mezclen entre sí. Cuando se sueltan todos los emisores, stdin se cierra y el CLI termina.
        let mut stdin = child.stdin.take().expect("stdin piped");
        let (wtx, mut wrx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Some(line) = wrx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() || stdin.write_all(b"\n").await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });
        let user = serde_json::json!({"type": "user", "message": {"role": "user", "content": fold_history(&req)}});
        let mut wtx = Some(wtx);
        let _ = wtx.as_ref().expect("writer").send(user.to_string());

        let mut stderr = child.stderr.take().expect("stderr piped");
        let stderr_task = tokio::spawn(async move {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s).await;
            s
        });

        let mut lines = BufReader::new(child.stdout.take().expect("stdout piped")).lines();
        let mut terminal = false;
        while let Some(line) = lines.next_line().await? {
            if let Some(cr) = parse_control_request(&line) {
                if cr.is_memory_edit(req.cwd.as_deref()) {
                    if let Some(w) = &wtx {
                        let _ = w.send(control_response(&cr, Some(Answer::Allow { always: false })));
                    }
                    continue;
                }
                let event = if cr.is_question() {
                    ProviderEvent::Question { id: cr.request_id.clone(), questions: cr.questions() }
                } else {
                    ProviderEvent::Permission {
                        id: cr.request_id.clone(),
                        tool: cr.tool.clone(),
                        input: cr.input.clone(),
                        description: cr.description.clone(),
                        can_accept_edits: cr.can_accept_edits,
                    }
                };
                // Se registra antes de avisar a la interfaz, para que su respuesta nunca llegue antes.
                let waiting = req.interactions.as_ref().map(|i| i.register(&cr.request_id));
                if tx.send(event).await.is_err() {
                    return Ok(());
                }
                if let Some(w) = wtx.clone() {
                    tokio::spawn(async move {
                        let answer = match waiting {
                            Some(rx) => rx.await.ok(),
                            None => None,
                        };
                        let _ = w.send(control_response(&cr, answer));
                    });
                }
                continue;
            }
            for ev in parse_line(&line) {
                let ends = matches!(ev, ProviderEvent::Finished | ProviderEvent::LimitReached { .. } | ProviderEvent::Error(_));
                terminal |= ends;
                if tx.send(ev).await.is_err() {
                    return Ok(());
                }
                if ends {
                    wtx = None; // el turno terminó: se cierra stdin para que el CLI salga
                }
            }
        }
        let status = child.wait().await?;
        let stderr = stderr_task.await.unwrap_or_default();
        if !terminal {
            let tail: String = stderr.trim().chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
            let _ = tx
                .send(ProviderEvent::Error(format!("claude terminó sin resultado ({status}): {tail}")))
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Líneas reales capturadas del CLI (ver docs/PROVIDER-FINDINGS.md).
    const WRITE: &str = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Write","display_name":"Write","input":{"file_path":"/p/hola.txt","content":"hola"},"description":"hola.txt","permission_suggestions":[{"type":"setMode","mode":"acceptEdits","destination":"session"}],"tool_use_id":"t1"}}"#;
    const ASK: &str = r#"{"type":"control_request","request_id":"r2","request":{"subtype":"can_use_tool","tool_name":"AskUserQuestion","display_name":"AskUserQuestion","input":{"questions":[{"question":"¿Prefieres rojo o azul?","header":"Color","options":[{"label":"Rojo","description":"Elegir rojo"},{"label":"Azul","description":"Elegir azul"}],"multiSelect":false}]},"tool_use_id":"t2","requires_user_interaction":true}}"#;

    #[test]
    fn parses_permission_and_question_requests() {
        let w = parse_control_request(WRITE).unwrap();
        assert_eq!((w.request_id.as_str(), w.tool.as_str()), ("r1", "Write"));
        assert!(w.can_accept_edits && !w.is_question());
        assert_eq!(w.input["file_path"], "/p/hola.txt");

        let q = parse_control_request(ASK).unwrap();
        assert!(q.is_question());
        let qs = q.questions();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].options[1].label, "Azul");
        assert!(!qs[0].multi_select);

        assert!(parse_control_request(r#"{"type":"assistant"}"#).is_none());
        assert!(parse_control_request("no es json").is_none());
        assert!(parse_control_request(r#"{"type":"control_request","request_id":"x","request":{"subtype":"otra"}}"#).is_none());
    }

    #[test]
    fn memory_file_edits_are_allowed_without_asking() {
        let mem = |p: &str| ControlRequest {
            request_id: "r".into(),
            tool: "Edit".into(),
            input: serde_json::json!({"file_path": p}),
            description: None,
            can_accept_edits: true,
        };
        let cwd = std::path::Path::new("/p/proyecto");
        assert!(mem("/p/proyecto/.comodel/MEMORY.md").is_memory_edit(Some(cwd)));
        assert!(!mem("/p/proyecto/MEMORY.md").is_memory_edit(Some(cwd)), "otro MEMORY.md no cuenta");
        assert!(!mem("/otro/.comodel/MEMORY.md").is_memory_edit(Some(cwd)), "fuera de la carpeta no cuenta");
        let mut bash = mem("/p/proyecto/.comodel/MEMORY.md");
        bash.tool = "Bash".into();
        assert!(!bash.is_memory_edit(Some(cwd)));
    }

    fn body(line: &str) -> Value {
        let v: Value = serde_json::from_str(line).unwrap();
        assert_eq!(v["type"], "control_response");
        v["response"].clone()
    }

    #[test]
    fn builds_the_responses_the_cli_expects() {
        let w = parse_control_request(WRITE).unwrap();
        let allow = body(&control_response(&w, Some(Answer::Allow { always: false })));
        assert_eq!(allow["request_id"], "r1");
        assert_eq!(allow["response"]["behavior"], "allow");
        assert_eq!(allow["response"]["updatedInput"]["content"], "hola");
        assert!(allow["response"].get("updatedPermissions").is_none());

        let always = body(&control_response(&w, Some(Answer::Allow { always: true })));
        assert_eq!(always["response"]["updatedPermissions"][0]["mode"], "acceptEdits");

        let deny = body(&control_response(&w, Some(Answer::Deny { message: None })));
        assert_eq!(deny["response"]["behavior"], "deny");
        assert!(deny["response"]["message"].as_str().unwrap().contains("rechaz"));

        // Sin nadie a quien preguntar, se rechaza.
        assert_eq!(body(&control_response(&w, None))["response"]["behavior"], "deny");

        let q = parse_control_request(ASK).unwrap();
        let mut answers = std::collections::HashMap::new();
        answers.insert("¿Prefieres rojo o azul?".to_string(), "Azul".to_string());
        let ans = body(&control_response(&q, Some(Answer::Answers { answers })));
        assert_eq!(ans["response"]["behavior"], "allow");
        assert_eq!(ans["response"]["updatedInput"]["answers"]["¿Prefieres rojo o azul?"], "Azul");
        assert_eq!(ans["response"]["updatedInput"]["questions"][0]["header"], "Color");
        // Un "permitir" a una pregunta no la contesta: se rechaza.
        assert_eq!(body(&control_response(&q, Some(Answer::Allow { always: false })))["response"]["behavior"], "deny");
    }
}
