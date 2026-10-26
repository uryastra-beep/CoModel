//! Importar un chat desde un enlace público de ChatGPT o de Claude.
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Debug, PartialEq)]
pub struct Imported {
    pub title: String,
    /// "chatgpt" o "claude": de dónde vienen los mensajes del asistente.
    pub source: String,
    pub messages: Vec<(bool, String)>, // (es del usuario, texto)
}

enum Link {
    ChatGpt(String),
    Claude(String),
}

fn parse_link(url: &str) -> Result<Link, String> {
    let url = url.trim();
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).ok_or("Pega el enlace completo, empezando por https://")?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.trim_start_matches("www.");
    let id = path.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    let id = id.strip_prefix("share/").map(|s| s.strip_prefix("e/").unwrap_or(s));
    let valid = |s: &str| !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    match (host, id) {
        ("chatgpt.com" | "chat.openai.com", Some(i)) if valid(i) => Ok(Link::ChatGpt(i.to_string())),
        ("claude.ai", Some(i)) if valid(i) => Ok(Link::Claude(i.to_string())),
        _ => Err("Ese enlace no parece un chat compartido de ChatGPT (chatgpt.com/share/...) ni de Claude (claude.ai/share/...).".into()),
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36")
        .build()
        .unwrap_or_default()
}

async fn get_json(url: &str) -> Result<Value, (u16, String)> {
    let res = client().get(url).header("accept", "application/json").send().await.map_err(|_| (0, "No hay conexión.".to_string()))?;
    let status = res.status().as_u16();
    let text = res.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) if status < 400 => Ok(v),
        _ => Err((status, text)),
    }
}

pub async fn import(url: &str) -> Result<Imported, String> {
    match parse_link(url)? {
        Link::ChatGpt(id) => {
            let v = get_json(&format!("https://chatgpt.com/backend-api/share/{id}")).await.map_err(|(s, body)| {
                if body.contains("not_found") || body.contains("deleted") || s == 404 {
                    "Ese chat ya no está compartido o el enlace es incorrecto.".to_string()
                } else {
                    "ChatGPT no dejó descargar ese chat. Inténtalo de nuevo en un rato.".to_string()
                }
            })?;
            from_chatgpt(&v)
        }
        Link::Claude(id) => {
            let v = get_json(&format!("https://claude.ai/api/chat_snapshots/{id}?rendering_mode=messages&render_all_tools=true"))
                .await
                .map_err(|(s, _)| {
                    if s == 403 || s == 429 {
                        "Claude bloquea la descarga automática de chats compartidos. Importa uno de ChatGPT, o copia la conversación y pégala en un chat nuevo.".to_string()
                    } else if s == 404 {
                        "Ese chat ya no está compartido o el enlace es incorrecto.".to_string()
                    } else {
                        "No se pudo descargar el chat de Claude.".to_string()
                    }
                })?;
            from_claude(&v)
        }
    }
}

fn from_chatgpt(v: &Value) -> Result<Imported, String> {
    let mapping = v.get("mapping").and_then(|m| m.as_object()).ok_or("ChatGPT devolvió el chat en un formato que no entiendo.")?;
    // Se sigue la rama activa desde el último mensaje hasta la raíz.
    let mut chain = Vec::new();
    let mut cur = v.get("current_node").and_then(|c| c.as_str()).map(String::from);
    while let Some(id) = cur {
        let Some(node) = mapping.get(&id) else { break };
        chain.push(node);
        cur = node.get("parent").and_then(|p| p.as_str()).map(String::from);
        if chain.len() > 5000 {
            break;
        }
    }
    chain.reverse();
    let mut messages = Vec::new();
    for node in chain {
        let Some(m) = node.get("message") else { continue };
        let role = m.pointer("/author/role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "user" && role != "assistant" {
            continue;
        }
        if m.pointer("/metadata/is_visually_hidden_from_conversation").and_then(|b| b.as_bool()) == Some(true) {
            continue;
        }
        let text = m
            .pointer("/content/parts")
            .and_then(|p| p.as_array())
            .map(|parts| parts.iter().filter_map(|p| p.as_str()).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        if !text.trim().is_empty() {
            messages.push((role == "user", text.trim().to_string()));
        }
    }
    finish(v.get("title").and_then(|t| t.as_str()), "chatgpt", messages)
}

fn from_claude(v: &Value) -> Result<Imported, String> {
    let list = v.get("chat_messages").and_then(|m| m.as_array()).ok_or("Claude devolvió el chat en un formato que no entiendo.")?;
    let mut messages = Vec::new();
    for m in list {
        let user = match m.get("sender").and_then(|s| s.as_str()) {
            Some("human") => true,
            Some("assistant") => false,
            _ => continue,
        };
        let text = match m.get("content").and_then(|c| c.as_array()) {
            Some(parts) => parts.iter().filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text")).filter_map(|p| p.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n"),
            None => m.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        };
        if !text.trim().is_empty() {
            messages.push((user, text.trim().to_string()));
        }
    }
    finish(v.get("snapshot_name").and_then(|t| t.as_str()).or_else(|| v.get("name").and_then(|t| t.as_str())), "claude", messages)
}

fn finish(title: Option<&str>, source: &str, messages: Vec<(bool, String)>) -> Result<Imported, String> {
    if messages.is_empty() {
        return Err("Ese chat no tiene mensajes de texto que importar.".into());
    }
    let title: String = title.map(str::trim).filter(|t| !t.is_empty()).unwrap_or("Chat importado").chars().take(40).collect();
    Ok(Imported { title, source: source.into(), messages })
}

/// Un archivo de sesión de Claude Code (`.jsonl`, uno por sesión en `~/.claude/projects/`).
pub fn from_claude_code(text: &str) -> Result<Imported, String> {
    let mut messages = Vec::new();
    let mut title: Option<String> = None;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if kind == "custom-title" {
            title = v.get("customTitle").and_then(|t| t.as_str()).map(String::from).or(title);
            continue;
        }
        if (kind != "user" && kind != "assistant")
            || v.get("isSidechain").and_then(|b| b.as_bool()) == Some(true)
            || v.get("isMeta").and_then(|b| b.as_bool()) == Some(true)
        {
            continue;
        }
        let content = v.pointer("/message/content");
        let text = match content {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(parts)) => parts
                .iter()
                .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => continue,
        };
        let t = text.trim();
        // Avisos internos de la herramienta, no parte de la conversación.
        if t.is_empty() || ["<local-command", "<command-", "<system-reminder", "Caveat:", "[Request interrupted"].iter().any(|p| t.starts_with(p)) {
            continue;
        }
        messages.push((kind == "user", t.to_string()));
    }
    let title = title.or_else(|| messages.iter().find(|m| m.0).map(|m| m.1.lines().next().unwrap_or("").to_string()));
    finish(title.as_deref(), "claude", messages)
}

pub fn import_file(path: &str) -> Result<Imported, String> {
    let meta = std::fs::metadata(path).map_err(|_| "No se pudo abrir ese archivo.".to_string())?;
    if !meta.is_file() || meta.len() > 100 * 1024 * 1024 {
        return Err("Ese archivo no es válido o pesa más de 100 MB.".into());
    }
    let text = std::fs::read_to_string(path).map_err(|_| "Ese archivo no es de texto.".to_string())?;
    from_claude_code(&text).map_err(|e| {
        if e.starts_with("Ese chat no tiene") { "No encontré una conversación de Claude Code en ese archivo. Busca el archivo .jsonl de la sesión.".into() } else { e }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn links_are_recognised_and_others_rejected() {
        assert!(matches!(parse_link("https://chatgpt.com/share/68a1b2c3-0000-4000-8000-123456789abc"), Ok(Link::ChatGpt(_))));
        assert!(matches!(parse_link("https://chatgpt.com/share/e/68a1b2c3-0000-4000-8000-123456789abc?x=1"), Ok(Link::ChatGpt(i)) if i.starts_with("68a1")));
        assert!(matches!(parse_link("https://claude.ai/share/abc-123"), Ok(Link::Claude(_))));
        assert!(parse_link("https://evil.com/share/abc").is_err());
        assert!(parse_link("https://chatgpt.com.evil.com/share/abc").is_err());
        assert!(parse_link("chatgpt.com/share/abc").is_err());
    }

    #[test]
    fn chatgpt_branch_is_followed_in_order_and_hidden_skipped() {
        let v = json!({"title":"Plan","current_node":"c","mapping":{
            "a":{"parent":null,"message":{"author":{"role":"system"},"content":{"parts":["x"]}}},
            "b":{"parent":"a","message":{"author":{"role":"user"},"content":{"parts":["hola"]}}},
            "h":{"parent":"b","message":{"author":{"role":"assistant"},"metadata":{"is_visually_hidden_from_conversation":true},"content":{"parts":["oculto"]}}},
            "c":{"parent":"h","message":{"author":{"role":"assistant"},"content":{"parts":["qué tal"]}}},
            "z":{"parent":"b","message":{"author":{"role":"assistant"},"content":{"parts":["otra rama"]}}}}});
        let i = from_chatgpt(&v).unwrap();
        assert_eq!(i.messages, vec![(true, "hola".to_string()), (false, "qué tal".to_string())]);
        assert_eq!(i.title, "Plan");
    }

    #[test]
    fn claude_code_session_keeps_text_and_skips_tools_and_notices() {
        let lines = [
            r#"{"type":"queue-operation"}"#,
            r#"{"type":"user","isSidechain":false,"message":{"role":"user","content":"hola"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<local-command-caveat>x</local-command-caveat>"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"buenas"},{"type":"tool_use","name":"Bash"}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"de un agente"}]}}"#,
            "no es json",
            r#"{"type":"custom-title","customTitle":"Mi sesión"}"#,
        ]
        .join("\n");
        let i = from_claude_code(&lines).unwrap();
        assert_eq!(i.messages, vec![(true, "hola".to_string()), (false, "buenas".to_string())]);
        assert_eq!(i.title, "Mi sesión");
        assert!(from_claude_code("{}\nnada").is_err());
    }

    /// Con `COMODEL_TEST_JSONL=ruta/a/sesion.jsonl` comprueba el formato real; sin la variable no hace nada.
    #[test]
    fn real_claude_code_session_if_given() {
        let Ok(path) = std::env::var("COMODEL_TEST_JSONL") else { return };
        let i = import_file(&path).unwrap();
        eprintln!("{} mensajes, título {:?}, primero: {:?}", i.messages.len(), i.title, i.messages.first().map(|m| m.1.chars().take(60).collect::<String>()));
        assert!(i.messages.len() > 2);
    }

    #[test]
    fn claude_snapshot_text_blocks_are_joined() {
        let v = json!({"snapshot_name":"Idea","chat_messages":[
            {"sender":"human","content":[{"type":"text","text":"hola"}]},
            {"sender":"assistant","content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"buenas"}]}]});
        let i = from_claude(&v).unwrap();
        assert_eq!(i.messages.len(), 2);
        assert_eq!(i.messages[1].1, "buenas");
    }
}
