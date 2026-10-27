use serde::{Deserialize, Serialize};

/// Evento normalizado que emite cualquier proveedor, sea CLI o API.
/// En JSON: `{"type": "Text", "data": "..."}` (lo que lee la interfaz).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ProviderEvent {
    /// El proveedor arrancó una sesión.
    Started {
        session_id: Option<String>,
        model: Option<String>,
        mode: Option<String>,
    },
    /// Fragmento de texto de la respuesta.
    Text(String),
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    /// El agente creó o editó un archivo.
    FileTouched { path: String },
    /// La IA entrega un archivo en su respuesta. Con `data` (base64) CoModel lo guarda en
    /// `.comodel/attachments/`; con `path` solo vale si está dentro de la carpeta de trabajo.
    /// No llega tal cual a la interfaz: el orquestador lo valida y emite `UiEvent::Outputs`.
    Attachment {
        name: String,
        #[serde(default)]
        mime: Option<String>,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        data: Option<String>,
    },
    /// Tokens usados en el turno y tamaño de la ventana de contexto, si se conoce.
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        context_window: Option<u64>,
        cost_usd: Option<f64>,
    },
    /// Uso restante: `used_fraction` va de 0.0 a 1.0.
    Quota {
        window: String,
        used_fraction: f64,
        resets_at: Option<u64>,
    },
    /// El proveedor ya no puede atender más peticiones (dispara el relevo).
    LimitReached {
        reason: String,
        resets_at: Option<u64>,
    },
    /// La IA pide permiso para usar una herramienta (editar un archivo, ejecutar un comando...).
    /// Se responde con `Interactions::answer`.
    Permission {
        id: String,
        tool: String,
        input: serde_json::Value,
        description: Option<String>,
        /// El CLI ofrece "permitir todas las ediciones de esta sesión".
        can_accept_edits: bool,
    },
    /// La IA le hace una pregunta al usuario. Se responde con `Answer::Answers`.
    Question { id: String, questions: Vec<AskQuestion> },
    Error(String),
    Finished,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskOption {
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskQuestion {
    pub question: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub options: Vec<AskOption>,
    #[serde(default, rename = "multiSelect")]
    pub multi_select: bool,
}
