use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::events::ProviderEvent;
use crate::interactions::Interactions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    /// Proveedor que escribió el mensaje (solo asistentes).
    pub provider: Option<String>,
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
}

/// Un turno que CoModel pide a un proveedor. CoModel es dueño del estado:
/// el proveedor no necesita recordar nada entre turnos.
#[derive(Debug, Clone, Default)]
pub struct TurnRequest {
    pub prompt: String,
    pub attachments: Vec<PathBuf>,
    /// Instrucciones del sistema (memoria compartida, paquete de relevo).
    pub preamble: Option<String>,
    /// Conversación previa, sin incluir el `prompt` actual.
    pub history: Vec<ChatMessage>,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Modo de permisos (solo CLI): default, plan, acceptEdits, auto...
    pub mode: Option<String>,
    pub cwd: Option<PathBuf>,
    /// Por dónde llegan las respuestas del usuario a permisos y preguntas. Sin esto, se rechazan.
    pub interactions: Option<Interactions>,
    /// Sesión del CLI de este chat que se puede continuar. Si falla (ya no existe o no cabe más),
    /// el adaptador empieza una nueva con `history`.
    pub resume: Option<Resume>,
}

/// Una sesión del CLI que ya conoce la conversación hasta cierto punto.
#[derive(Debug, Clone, PartialEq)]
pub struct Resume {
    pub session_id: String,
    /// Lo ocurrido en el chat desde el último turno de esa sesión (lo que otra IA escribió, por ejemplo).
    pub missed: Vec<ChatMessage>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("no se pudo lanzar el proveedor: {0}")]
    Spawn(String),
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("error de red: {0}")]
    Http(String),
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;

    /// `true` si la IA puede abrir archivos por su cuenta (Claude Code). Si no, CoModel le pega
    /// el contenido de los archivos adjuntos dentro del mensaje.
    fn reads_files(&self) -> bool {
        true
    }

    /// La IA guarda sesiones que CoModel puede continuar en el turno siguiente.
    fn resumable(&self) -> bool {
        false
    }

    /// El adaptador recibe adjuntos estructurados en vez de texto pegado.
    fn native_attachments(&self) -> bool { false }

    /// Ejecuta un turno y emite eventos por `tx`. Un límite de uso se avisa con
    /// `ProviderEvent::LimitReached`, no con `Err`.
    async fn run_turn(
        &self,
        req: TurnRequest,
        tx: mpsc::Sender<ProviderEvent>,
    ) -> Result<(), ProviderError>;
}

/// Convierte el historial + prompt en un único texto, para proveedores sin sesión.
pub fn fold_history(req: &TurnRequest) -> String {
    fold_messages(&req.history, &req.prompt)
}

/// Como `fold_history`, con los mensajes que se indiquen.
pub fn fold_messages(history: &[ChatMessage], prompt: &str) -> String {
    if history.is_empty() {
        return prompt.to_string();
    }
    let mut out = String::from("## Conversación previa\n");
    for m in history {
        out.push_str(&format!("[{}] {}\n", m.role.as_str(), m.content));
    }
    out.push_str("\n## Mensaje actual\n");
    out.push_str(prompt);
    out
}
