//! Prueba técnica del núcleo.
//!   spike claude "<prompt>"      un turno real con Claude Code
//!   spike openrouter "<prompt>"  un turno real con OpenRouter (usa OPENROUTER_API_KEY)
//!   spike codex "<prompt>"       un turno real con Codex (sesión de ChatGPT, sin clave)
//!   spike demo                   relevo real: Claude (límite simulado) -> segundo Claude
//!   spike perm [prompt] [deny]   Claude pide permiso a mitad del turno y se responde solo

use std::sync::Arc;

use comodel_core::claude::ClaudeCode;
use comodel_core::events::ProviderEvent;
use comodel_core::interactions::{Answer, Interactions};
use comodel_core::memory::Memory;
use comodel_core::openrouter::OpenRouter;
use comodel_core::orchestrator::{Orchestrator, UiEvent};
use comodel_core::provider::Provider;
use comodel_core::sim::LimitOnCall;
use tokio::sync::mpsc;

fn print_event(provider: &str, ev: &ProviderEvent) {
    match ev {
        ProviderEvent::Text(t) => println!("[{provider}] texto: {t}"),
        ProviderEvent::ToolUse { name, .. } => println!("[{provider}] herramienta: {name}"),
        other => println!("[{provider}] {other:?}"),
    }
}

async fn drain(mut rx: mpsc::UnboundedReceiver<UiEvent>) {
    while let Some(ev) = rx.recv().await {
        match ev {
            UiEvent::Provider { provider, event } => print_event(&provider, &event),
            UiEvent::Handoff { from, to, reason } => println!("=== RELEVO {from} -> {to} ({reason}) ==="),
        }
    }
}

fn claude(id: &str) -> Arc<ClaudeCode> {
    let mut c = ClaudeCode::new(id);
    // Sin herramientas por defecto: la prueba solo conversa. COMODEL_TOOLS=1 las activa.
    if std::env::var("COMODEL_TOOLS").is_err() {
        c.tools = Some(String::new());
    }
    Arc::new(c)
}

fn temp_project(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("comodel-spike-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (ui, rx) = mpsc::unbounded_channel();
    let printer = tokio::spawn(drain(rx));

    match args.first().map(String::as_str) {
        Some("claude") => {
            let prompt = args.get(1).cloned().unwrap_or_else(|| "responde solo: ok".into());
            let dir = temp_project("claude");
            let mut orch = Orchestrator::new(vec![claude("claude")], Memory::open(&dir, "spike").unwrap());
            orch.options.model = Some("haiku".into());
            orch.options.mode = std::env::var("COMODEL_MODE").ok();
            orch.options.cwd = Some(dir.clone());
            let result = orch.send(&prompt, &ui).await;
            println!("\n--- MEMORY.md tras el turno ---\n{}", orch.memory().read_md().unwrap());
            println!("archivos en {}: {:?}", dir.display(), std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect::<Vec<_>>());
            match result {
                Ok(r) => println!("\nrespuesta final de {}: {}", r.provider, r.text),
                Err(e) => println!("\nERROR: {e}"),
            }
        }
        Some("codex") => {
            let prompt = args.get(1).cloned().unwrap_or_else(|| "Crea nota.txt con el texto: prueba".into());
            let dir = temp_project("codex");
            let bin = comodel_core::codex::find_binary().expect("no se encontró codex");
            let mut orch = Orchestrator::new(vec![Arc::new(comodel_core::codex::Codex::new("codex", bin, false))], Memory::open(&dir, "spike").unwrap());
            orch.options.cwd = Some(dir.clone());
            orch.options.memory_instruction = true;
            let result = orch.send(&prompt, &ui).await;
            println!("\n--- MEMORY.md tras el turno ---\n{}", orch.memory().read_md().unwrap());
            println!("archivos en {}: {:?}", dir.display(), std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect::<Vec<_>>());
            match result {
                Ok(r) => println!("\nrespuesta final de {}: {}", r.provider, r.text),
                Err(e) => println!("\nERROR: {e}"),
            }
        }
        Some("perm") => {
            // Claude pide permiso a mitad del turno; aquí se responde solo para comprobar el circuito completo.
            let prompt = args.get(1).cloned().unwrap_or_else(|| "Crea un archivo hola.txt con el texto hola.".into());
            let deny = args.get(2).map(String::as_str) == Some("deny");
            let dir = temp_project("perm");
            let inter = Interactions::new();
            let mut orch = Orchestrator::new(vec![Arc::new(ClaudeCode::new("claude"))], Memory::open(&dir, "perm").unwrap());
            orch.options.model = Some("haiku".into());
            orch.options.mode = Some("default".into());
            orch.options.cwd = Some(dir.clone());
            orch.options.interactions = Some(inter.clone());
            let (ui2, mut rx2) = mpsc::unbounded_channel();
            let answerer = tokio::spawn(async move {
                while let Some(ev) = rx2.recv().await {
                    if let UiEvent::Provider { event, .. } = ev {
                        match &event {
                            ProviderEvent::Permission { id, tool, .. } => {
                                println!("PERMISO pedido para {tool}: {}", if deny { "rechazo" } else { "permito" });
                                inter.answer(id, if deny { Answer::Deny { message: None } } else { Answer::Allow { always: false } });
                            }
                            ProviderEvent::Question { id, questions } => {
                                let mut answers = std::collections::HashMap::new();
                                for q in questions {
                                    answers.insert(q.question.clone(), q.options.first().map(|o| o.label.clone()).unwrap_or_default());
                                }
                                println!("PREGUNTA: {answers:?}");
                                inter.answer(id, Answer::Answers { answers });
                            }
                            ProviderEvent::Text(t) => println!("texto: {t}"),
                            _ => {}
                        }
                    }
                }
            });
            let r = orch.send(&prompt, &ui2).await;
            drop(ui2);
            let _ = answerer.await;
            println!("resultado: {:?}", r.map(|x| x.text));
            let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
            println!("archivos en {}: {files:?}", dir.display());
        }
        Some("openrouter") => {
            let Ok(key) = std::env::var("OPENROUTER_API_KEY") else {
                println!("Falta OPENROUTER_API_KEY en el entorno. No la pegues en chats ni archivos.");
                std::process::exit(2);
            };
            let prompt = args.get(1).cloned().unwrap_or_else(|| "responde solo: ok".into());
            let dir = temp_project("openrouter");
            let mut orch = Orchestrator::new(vec![Arc::new(OpenRouter::new("openrouter", key))], Memory::open(&dir, "spike").unwrap());
            match orch.send(&prompt, &ui).await {
                Ok(r) => println!("\nrespuesta final de {}: {}", r.provider, r.text),
                Err(e) => println!("\nERROR: {e}"),
            }
        }
        Some("demo") => {
            let dir = temp_project("demo");
            // A se agota en su 2.ª llamada; B (otro Claude) debe seguir con el contexto.
            let a: Arc<dyn Provider> = Arc::new(LimitOnCall::new(claude("claude-a"), 2));
            let b: Arc<dyn Provider> = claude("claude-b");
            let mut orch = Orchestrator::new(vec![a, b], Memory::open(&dir, "spike").unwrap());
            orch.options.model = Some("haiku".into());
            orch.options.cwd = Some(dir.clone());
            for msg in [
                "Me llamo Ury y estamos construyendo una app llamada CoModel. Responde solo: entendido.",
                "¿Cómo me llamo y qué app estamos construyendo? Responde en una frase.",
            ] {
                println!("\n>>> usuario: {msg}");
                match orch.send(msg, &ui).await {
                    Ok(r) => println!("<<< {} respondió: {}", r.provider, r.text.trim()),
                    Err(e) => println!("ERROR: {e}"),
                }
            }
            println!("\n--- MEMORY.md ---\n{}", orch.memory().read_md().unwrap());
            println!("relevos registrados: {}", orch.memory().handoff_count().unwrap());
        }
        _ => println!("uso: spike claude|openrouter [prompt] | spike demo"),
    }
    drop(ui);
    let _ = printer.await;
}
