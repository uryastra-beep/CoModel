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
    if req.history.is_empty() {
        return req.prompt.clone();
    }
    let mut out = String::from("## Conversación previa\n");
    for m in &req.history {
        out.push_str(&format!("[{}] {}\n", m.role.as_str(), m.content));
    }
    out.push_str("\n## Mensaje actual\n");
    out.push_str(&req.prompt);
    out
}
