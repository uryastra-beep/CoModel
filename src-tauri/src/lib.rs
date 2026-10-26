mod account;
mod chatgpt_auth;
mod import;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use comodel_core::claude::ClaudeCode;
use comodel_core::files::{self, Entry, FileContent, FileError, GitChange};
use comodel_core::interactions::{Answer, Interactions};
use comodel_core::memory::{Memory, UsageSummary};
use comodel_core::openrouter::OpenRouter;
use comodel_core::orchestrator::{Orchestrator, UiEvent};
use comodel_core::projects::{Project, ProjectStore, ProjectsFile};
use comodel_core::provider::{ChatMessage, Provider};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, Mutex};

/// Nombre del evento que recibe la interfaz (ver `src/lib/backend.ts`).
const UI_EVENT: &str = "comodel://event";

/// Servicio con el que se guardan las claves en el llavero del sistema (Secret Service en Linux).
const KEY_SERVICE: &str = "comodel";

/// Un chat del lienzo: su propio orquestador (historial y proveedores agotados). Puede estar ligado a un
/// proyecto (una carpeta) o no. Los chats de un mismo proyecto comparten su memoria (`MEMORY.md`, archivos, relevos).
struct ChatEntry {
    project_id: Option<String>,
    orch: Arc<Mutex<Orchestrator>>,
}

struct AppState {
    projects: Mutex<ProjectStore>,
    chats: Mutex<HashMap<String, ChatEntry>>,
    /// Donde viven los chats sin carpeta: su historial y su memoria, sin acceso a archivos del usuario.
    general_dir: PathBuf,
    /// Permisos y preguntas de las IAs que esperan la respuesta del usuario.
    interactions: Interactions,
    /// Mensajes en curso, para poder pararlos.
    running: Mutex<HashMap<String, tokio::task::AbortHandle>>,
    /// Lista de modelos de OpenRouter, pedida una vez por sesión.
    openrouter_models: Mutex<Option<Vec<OpenRouterModel>>>,
}

/// Dónde trabaja un chat: la carpeta de un proyecto o el espacio general (sin carpeta).
struct Workspace {
    path: PathBuf,
    relay: Vec<String>,
    has_folder: bool,
}

#[derive(Deserialize)]
struct SendOptions {
    /// IA elegida en el chat; va primero y el resto del relevo queda detrás.
    #[serde(default)]
    provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    mode: Option<String>,
    title: Option<String>,
    /// Rutas absolutas de los archivos adjuntos al mensaje.
    #[serde(default)]
    attachments: Vec<String>,
    /// El texto es el foco de un `/compact`: se resume la conversación en vez de contestarla.
    #[serde(default)]
    compact: bool,
    #[serde(default)]
    #[serde(alias = "sharedChats")]
    shared_chats: Vec<SharedChat>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedChat {
    chat_id: String,
    project_id: Option<String>,
    title: String,
}

/// Evento hacia la interfaz, con el chat al que pertenece.
#[derive(Serialize, Clone)]
struct ChatEvent {
    chat_id: String,
    #[serde(flatten)]
    event: UiEvent,
}

const MAX_ATTACHMENTS: usize = 10;
const MAX_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;

/// Nombre de archivo seguro para guardar dentro del proyecto.
fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("archivo");
    let cleaned: String = base.chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let cleaned = cleaned.trim_start_matches('.').chars().take(80).collect::<String>();
    if cleaned.is_empty() { "archivo".into() } else { cleaned }
}

fn attachments_dir(root: &Path) -> Result<PathBuf, String> {
    let dir = root.join(".comodel").join("attachments");
    std::fs::create_dir_all(&dir).map_err(|_| "No se pudo preparar la carpeta de adjuntos.".to_string())?;
    Ok(dir)
}

fn stamp() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Valida los adjuntos y los deja dentro de la carpeta de trabajo: los que ya están ahí se usan tal cual
/// y los de fuera se copian a `.comodel/attachments/`, para que la IA pueda abrirlos sin permisos extra.
fn resolve_attachments(root: &Path, raw: &[String]) -> Result<Vec<PathBuf>, String> {
    if raw.len() > MAX_ATTACHMENTS {
        return Err(format!("Puedes adjuntar hasta {MAX_ATTACHMENTS} archivos por mensaje."));
    }
    let root = std::fs::canonicalize(root).map_err(|_| "No se pudo abrir la carpeta de trabajo.".to_string())?;
    let mut out = Vec::new();
    for r in raw {
        let path = std::fs::canonicalize(r).map_err(|_| "No se encontró uno de los archivos adjuntos.".to_string())?;
        let meta = std::fs::metadata(&path).map_err(|_| "No se pudo leer uno de los archivos adjuntos.".to_string())?;
        if !meta.is_file() {
            return Err("Solo se pueden adjuntar archivos, no carpetas.".into());
        }
        if meta.len() > MAX_ATTACHMENT_BYTES {
            return Err("Un adjunto pesa más de 20 MB.".into());
        }
        if path.starts_with(&root) {
            out.push(path);
        } else {
            let name = path.file_name().map(|n| safe_name(&n.to_string_lossy())).unwrap_or_else(|| "archivo".into());
            let dest = attachments_dir(&root)?.join(format!("{}-{name}", stamp()));
            std::fs::copy(&path, &dest).map_err(|_| "No se pudo copiar un adjunto al proyecto.".to_string())?;
            out.push(dest);
        }
    }
    Ok(out)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// El proyecto con ese id, o el espacio general si no se indica ninguno.
async fn workspace(state: &AppState, project_id: Option<&str>) -> Result<Workspace, String> {
    let store = state.projects.lock().await;
    match project_id {
        None => Ok(Workspace { path: state.general_dir.clone(), relay: store.state().default_relay.clone(), has_folder: false }),
        Some(id) => {
            if !valid_id(id) {
                return Err("identificador no válido".into());
            }
            let p = store.get(id).ok_or_else(|| "No se encontró el proyecto.".to_string())?;
            Ok(Workspace { path: p.path.clone(), relay: p.relay.clone(), has_folder: true })
        }
    }
}

/// Crea el adaptador de una IA, si se puede usar en este equipo. Sin carpeta ninguna toca archivos:
/// Claude sin herramientas, Codex en solo lectura.
async fn make_provider(id: &str, has_folder: bool) -> Option<Arc<dyn Provider>> {
    match id {
        "claude" => {
            let mut c = ClaudeCode::new("claude");
            if !has_folder {
                c.tools = Some(String::new());
            }
            Some(Arc::new(c))
        }
        "chatgpt" => { if chatgpt_auth::status().await.0 { Some(Arc::new(chatgpt_auth::ChatGPTProvider)) } else { None } },
        "openrouter" => stored_key("openrouter").await.map(|key| Arc::new(OpenRouter::new("openrouter", key)) as Arc<dyn Provider>),
        "codex" => comodel_core::codex::find_binary().map(|bin| Arc::new(comodel_core::codex::Codex::new("codex", bin, !has_folder)) as Arc<dyn Provider>),
        _ => None,
    }
}

/// Las IAs del chat, en su orden de relevo. Las que no se pueden usar (sin clave, sin instalar) se saltan.
async fn build_providers(relay: &[String], has_folder: bool) -> Vec<Arc<dyn Provider>> {
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    for id in relay {
        if let Some(p) = make_provider(id, has_folder).await {
            providers.push(p);
        }
    }
    if providers.is_empty() {
        providers.extend(make_provider("claude", has_folder).await);
    }
    providers
}

async fn chat(state: &AppState, project_id: Option<&str>, chat_id: &str) -> Result<Arc<Mutex<Orchestrator>>, String> {
    if !valid_id(chat_id) {
        return Err("identificador no válido".into());
    }
    let ws = workspace(state, project_id).await?;
    let mut chats = state.chats.lock().await;
    if let Some(c) = chats.get(chat_id) {
        if c.project_id.as_deref() == project_id {
            return Ok(c.orch.clone());
        }
        // El chat cambió de carpeta (solo es posible antes de su primer mensaje): se vuelve a crear.
    }
    let memory = Memory::open(&ws.path, chat_id).map_err(|e| e.to_string())?;
    let mut orch = Orchestrator::new(build_providers(&ws.relay, ws.has_folder).await, memory);
    orch.options.cwd = Some(ws.path.clone());
    orch.options.memory_instruction = ws.has_folder;
    orch.options.interactions = Some(state.interactions.clone());
    let orch = Arc::new(Mutex::new(orch));
    chats.insert(chat_id.to_string(), ChatEntry { project_id: project_id.map(String::from), orch: orch.clone() });
    Ok(orch)
}

/// Envía un mensaje al proveedor activo del chat. Los eventos llegan a la interfaz por
/// `UI_EVENT` mientras el turno corre; el resultado es el texto final.
#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: Option<String>,
    chat_id: String,
    text: String,
    options: SendOptions,
) -> Result<String, String> {
    let ws = workspace(&state, project_id.as_deref()).await?;
    let attachments = {
        let (root, raw) = (ws.path.clone(), options.attachments.clone());
        tauri::async_runtime::spawn_blocking(move || resolve_attachments(&root, &raw)).await.map_err(|e| e.to_string())??
    };
    let orch = chat(&state, project_id.as_deref(), &chat_id).await?;
    let mut shared_context = String::new();
    if options.shared_chats.len() > 32 { return Err("Conecta como máximo 32 chats por grupo.".into()); }
    for peer in &options.shared_chats {
        if peer.chat_id == chat_id { continue; }
        if !valid_id(&peer.chat_id) { return Err("Identificador de chat conectado no válido.".into()); }
        let peer_ws = workspace(&state, peer.project_id.as_deref()).await?;
        let peer_id = peer.chat_id.clone();
        let snapshot = tauri::async_runtime::spawn_blocking(move || {
            if !peer_ws.path.join(".comodel/memory.db").exists() { return Ok(String::new()); }
            Memory::open(&peer_ws.path, &peer_id).and_then(|m| m.shared_snapshot()).map_err(|e| e.to_string())
        }).await.map_err(|e| e.to_string())??;
        shared_context.push_str(&format!("\n\n## Chat conectado: {}\n{}", peer.title.chars().take(120).collect::<String>(), snapshot.chars().take(48000 / options.shared_chats.len().max(1)).collect::<String>()));
    }
    let chosen = match options.provider.as_deref() {
        Some(id) => Some(make_provider(id, ws.has_folder).await.ok_or_else(|| match id {
            "openrouter" => "OpenRouter no tiene clave. Guárdala en Ajustes, IAs.".to_string(),
            "chatgpt" => "Conecta ChatGPT en Ajustes → IAs y permite usar tu plan.".to_string(),
            "codex" => "No se encontró Codex en este equipo. Revisa Ajustes, IAs.".to_string(),
            _ => "Esa IA no está disponible.".to_string(),
        })?),
        None => None,
    };

    // El mensaje corre en su propia tarea para que se pueda parar desde la interfaz (`stop_message`).
    let id = chat_id.clone();
    let task = tokio::spawn(async move {
        let mut orch = orch.lock().await;
        orch.options.attachments = attachments;
        orch.options.shared_context = shared_context;
        if let Some(p) = chosen {
            orch.prefer(p);
        }
        orch.options.provider = options.provider;
        orch.options.model = options.model;
        orch.options.effort = options.effort;
        orch.options.mode = options.mode;
        orch.options.title = options.title;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let forward = tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                let _ = app.emit(UI_EVENT, ChatEvent { chat_id: id.clone(), event });
            }
        });
        let result = if options.compact { orch.compact(&text, &tx).await } else { orch.send(&text, &tx).await };
        drop(tx);
        let _ = forward.await;
        result.map(|r| r.text).map_err(|e| e.to_string())
    });
    state.running.lock().await.insert(chat_id.clone(), task.abort_handle());
    let outcome = task.await;
    state.running.lock().await.remove(&chat_id);
    match outcome {
        Ok(r) => r,
        Err(e) if e.is_cancelled() => Err("Detenido.".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Para el mensaje en curso de un chat. El proceso de la IA se cierra y lo pendiente se cancela.
#[tauri::command]
async fn stop_message(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    if let Some(h) = state.running.lock().await.remove(&chat_id) {
        h.abort();
    }
    Ok(())
}

/// La respuesta del usuario a un permiso o una pregunta de la IA.
#[tauri::command]
async fn answer_interaction(state: State<'_, AppState>, request_id: String, answer: Answer) -> Result<(), String> {
    if state.interactions.answer(&request_id, answer) {
        Ok(())
    } else {
        Err("Esa petición ya no está esperando respuesta.".into())
    }
}

/// Cambia el workspace sin borrar la conversación ni alterar los archivos del proyecto anterior.
#[tauri::command]
async fn move_chat_workspace(state: State<'_, AppState>, chat_id: String, from_project_id: Option<String>, to_project_id: Option<String>) -> Result<(), String> {
    if !valid_id(&chat_id) { return Err("Identificador de chat no válido.".into()); }
    if state.running.lock().await.contains_key(&chat_id) { return Err("Espera a que termine el chat antes de cambiar su carpeta.".into()); }
    let from = workspace(&state, from_project_id.as_deref()).await?;
    let to = workspace(&state, to_project_id.as_deref()).await?;
    let id = chat_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if from.path == to.path || !from.path.join(".comodel/memory.db").exists() { return Ok(()); }
        if std::fs::canonicalize(&from.path).ok() == std::fs::canonicalize(&to.path).ok() { return Ok(()); }
        Memory::open(&from.path, &id).and_then(|m| m.copy_chat_to(&to.path)).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())??;
    // El siguiente mensaje recreará el orquestador con el cwd, historial y permisos del destino.
    state.chats.lock().await.remove(&chat_id);
    Ok(())
}

/// Guarda una imagen pegada desde el portapapeles dentro de la carpeta de trabajo y devuelve su ruta.
#[tauri::command]
async fn save_pasted_image(state: State<'_, AppState>, project_id: Option<String>, name: String, data: String) -> Result<String, String> {
    use base64::Engine;
    let ws = workspace(&state, project_id.as_deref()).await?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| "La imagen pegada no es válida.".to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err("La imagen pegada está vacía o pesa más de 20 MB.".into());
    }
    let name = safe_name(&name);
    let ext_ok = ["png", "jpg", "jpeg", "gif", "webp"].iter().any(|e| name.to_lowercase().ends_with(&format!(".{e}")));
    let name = if ext_ok { name } else { format!("{name}.png") };
    tauri::async_runtime::spawn_blocking(move || {
        let dest = attachments_dir(&ws.path)?.join(format!("{}-{name}", stamp()));
        std::fs::write(&dest, bytes).map_err(|_| "No se pudo guardar la imagen pegada.".to_string())?;
        Ok(dest.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// La conversación guardada de un chat, para volver a mostrarla al reabrir la app.
#[tauri::command]
async fn chat_history(state: State<'_, AppState>, project_id: Option<String>, chat_id: String) -> Result<Vec<ChatMessage>, String> {
    if !valid_id(&chat_id) {
        return Err("identificador no válido".into());
    }
    let ws = workspace(&state, project_id.as_deref()).await?;
    // Sin base de datos todavía no hay historial: no se crea nada solo por mirar.
    if !ws.path.join(".comodel").join("memory.db").exists() {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        Memory::open(&ws.path, &chat_id).and_then(|m| m.recent_messages(500)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}


/// Guarda solo en el destino elegido en el diálogo nativo.
#[tauri::command]
async fn write_document(path: String, content: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || std::fs::write(path, content).map_err(|e| format!("No se pudo guardar: {e}")))
        .await.map_err(|e| e.to_string())?
}

/// Imágenes locales del proyecto o adjuntos elegidos por el usuario. Nunca devuelve texto ni HTML.
#[tauri::command]
async fn read_image(state: State<'_, AppState>, project_id: Option<String>, path: String) -> Result<String, String> {
    let ws = workspace(&state, project_id.as_deref()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = path.strip_prefix("file://").unwrap_or(&path);
        let full = PathBuf::from(path);
        let (root, rel) = if full.is_absolute() {
            (full.parent().ok_or("Ruta no válida.")?.to_path_buf(), full.file_name().ok_or("Ruta no válida.")?.to_string_lossy().to_string())
        } else { (ws.path, path.to_string()) };
        match files::read_file(&root, &rel).map_err(file_error)? {
            FileContent::Image { data, mime, .. } => Ok(format!("data:{mime};base64,{data}")),
            FileContent::Unsupported { reason, .. } => Err(reason),
            _ => Err("Ese archivo no es una imagen.".into()),
        }
    }).await.map_err(|e| e.to_string())?
}

// ---------- Archivos del proyecto ----------

/// La carpeta de un proyecto. Los archivos solo se tocan dentro de proyectos, nunca del espacio general.
async fn project_root(state: &AppState, project_id: &str) -> Result<PathBuf, String> {
    Ok(workspace(state, Some(project_id)).await?.path)
}

fn file_error(e: FileError) -> String {
    match e {
        FileError::Msg(m) => m,
        FileError::Conflict => "CONFLICT".into(),
    }
}

#[tauri::command]
async fn list_dir(state: State<'_, AppState>, project_id: String, path: String) -> Result<Vec<Entry>, String> {
    let root = project_root(&state, &project_id).await?;
    tauri::async_runtime::spawn_blocking(move || files::list_dir(&root, &path).map_err(file_error)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn read_file(state: State<'_, AppState>, project_id: String, path: String) -> Result<FileContent, String> {
    let root = project_root(&state, &project_id).await?;
    tauri::async_runtime::spawn_blocking(move || files::read_file(&root, &path).map_err(file_error)).await.map_err(|e| e.to_string())?
}

/// Fecha de modificación (ms), para detectar que un archivo abierto cambió en disco.
#[tauri::command]
async fn file_mtime(state: State<'_, AppState>, project_id: String, path: String) -> Result<u64, String> {
    let root = project_root(&state, &project_id).await?;
    tauri::async_runtime::spawn_blocking(move || files::mtime(&root, &path).map_err(file_error)).await.map_err(|e| e.to_string())?
}

/// Guarda un archivo. Con `expected_mtime`, falla con "CONFLICT" si cambió en disco desde que se abrió.
#[tauri::command]
async fn write_file(state: State<'_, AppState>, project_id: String, path: String, content: String, expected_mtime: Option<u64>) -> Result<u64, String> {
    let root = project_root(&state, &project_id).await?;
    tauri::async_runtime::spawn_blocking(move || files::write_file(&root, &path, &content, expected_mtime).map_err(file_error))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_status(state: State<'_, AppState>, project_id: String) -> Result<Vec<GitChange>, String> {
    let root = project_root(&state, &project_id).await?;
    tauri::async_runtime::spawn_blocking(move || files::git_status(&root)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn git_diff(state: State<'_, AppState>, project_id: String, path: String) -> Result<Option<String>, String> {
    let root = project_root(&state, &project_id).await?;
    tauri::async_runtime::spawn_blocking(move || files::git_diff(&root, &path)).await.map_err(|e| e.to_string())
}

// ---------- Proyectos ----------

#[tauri::command]
async fn list_projects(state: State<'_, AppState>) -> Result<ProjectsFile, String> {
    Ok(state.projects.lock().await.state().clone())
}

#[tauri::command]
async fn create_project(state: State<'_, AppState>, name: String, path: String, relay: Vec<String>) -> Result<Project, String> {
    state.projects.lock().await.create(&name, &PathBuf::from(path), relay).map_err(|e| e.to_string())
}

/// Asigna una carpeta persistente dentro de CoModel/Chats a un workspace sin proyecto (los antiguos "Chat #n" se siguen reconociendo).
#[tauri::command]
async fn create_chat_workspace(state: State<'_, AppState>, chat_id: String, title: String) -> Result<Project, String> {
    if !valid_id(&chat_id) { return Err("Identificador de chat no válido.".into()); }
    // La carpeta pertenece a esta instalación de CoModel, no al directorio desde el que se abra.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().ok_or("No se encontró la carpeta de CoModel.")?.join("Chats");
    let mut store = state.projects.lock().await;
    std::fs::create_dir_all(&root).map_err(|e| format!("No se pudo preparar CoModel/Chats: {e}"))?;
    let mut number: u64 = 0;
    let mut found = None;
    for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(n) = name.strip_prefix("Workspace #").or_else(|| name.strip_prefix("Chat #")).and_then(|s| s.split(" - ").next()).and_then(|s| s.parse::<u64>().ok()) { number = number.max(n); }
        let marker = entry.path().join(".comodel/workspace-chat.json");
        if let Ok(data) = std::fs::read_to_string(marker) {
            if serde_json::from_str::<serde_json::Value>(&data).ok().and_then(|v| v.get("chat_id").and_then(|id| id.as_str()).map(str::to_string)).as_deref() == Some(chat_id.as_str()) { found = Some(entry.path()); }
        }
    }
    let path = match found {
        Some(path) => path,
        None => {
            let title: String = title.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).take(50).collect();
            let title = title.trim().trim_end_matches('.');
            let title = if title.is_empty() { "Sin título" } else { title };
            let path = loop {
                number = number.checked_add(1).ok_or("No se pudo asignar un número al chat.")?;
                let candidate = root.join(format!("Workspace #{number} - {title}"));
                match std::fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(format!("No se pudo crear la carpeta del chat: {e}")),
                }
            };
            std::fs::create_dir_all(path.join(".comodel")).map_err(|e| e.to_string())?;
            std::fs::write(path.join(".comodel/workspace-chat.json"), serde_json::json!({ "chat_id": chat_id }).to_string()).map_err(|e| e.to_string())?;
            path
        }
    };
    if let Some(p) = store.find_by_path(&path) { return Ok(p.clone()); }
    let name = path.file_name().ok_or("Nombre de carpeta no válido.")?.to_string_lossy().to_string();
    let relay = store.state().default_relay.clone();
    store.create(&name, &path, relay).map_err(|e| e.to_string())
}

/// Abre una carpeta suelta como proyecto: si ya lo es, se devuelve el que existe; si no, se crea con el
/// nombre de la carpeta y el orden de relevo por defecto.
#[tauri::command]
async fn open_folder(state: State<'_, AppState>, path: String) -> Result<Project, String> {
    let mut store = state.projects.lock().await;
    let path = PathBuf::from(path);
    if let Some(p) = store.find_by_path(&path) {
        return Ok(p.clone());
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Proyecto".into());
    let relay = store.state().default_relay.clone();
    store.create(&name, &path, relay).map_err(|e| e.to_string())
}

/// Cambia el nombre o el orden de relevo. Los chats del proyecto se reconstruyen con las IAs nuevas
/// (su historial queda guardado).
#[tauri::command]
async fn update_project(state: State<'_, AppState>, id: String, name: Option<String>, relay: Option<Vec<String>>) -> Result<Project, String> {
    let p = state.projects.lock().await.update(&id, name.as_deref(), relay).map_err(|e| e.to_string())?;
    state.chats.lock().await.retain(|_, c| c.project_id.as_deref() != Some(id.as_str()));
    Ok(p)
}

/// Quita el proyecto de CoModel. No borra nada de la carpeta.
#[tauri::command]
async fn remove_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.projects.lock().await.remove(&id).map_err(|e| e.to_string())?;
    state.chats.lock().await.retain(|_, c| c.project_id.as_deref() != Some(id.as_str()));
    Ok(())
}

/// Orden de relevo de los chats sin carpeta.
#[tauri::command]
async fn set_default_relay(state: State<'_, AppState>, relay: Vec<String>) -> Result<(), String> {
    state.projects.lock().await.set_default_relay(relay).map_err(|e| e.to_string())?;
    state.chats.lock().await.retain(|_, c| c.project_id.is_some());
    Ok(())
}

// ---------- Claves de API ----------

/// Solo OpenRouter usa clave por ahora. Claude Code usa la sesión del propio CLI.
fn key_entry(provider: &str) -> Result<keyring::Entry, String> {
    if provider != "openrouter" {
        return Err("Ese proveedor no usa clave de API.".into());
    }
    keyring::Entry::new(KEY_SERVICE, provider).map_err(keyring_error)
}

/// Mensajes pensados para el usuario; nunca incluyen la clave.
pub(crate) fn keyring_error(e: keyring::Error) -> String {
    match e {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => {
            "No se pudo acceder al llavero del sistema. Comprueba que gnome-keyring o KWallet está activo y desbloqueado.".into()
        }
        _ => "No se pudo guardar la clave de forma segura.".into(),
    }
}

async fn stored_key(provider: &str) -> Option<String> {
    let provider = provider.to_string();
    tauri::async_runtime::spawn_blocking(move || key_entry(&provider).ok()?.get_password().ok())
        .await
        .ok()
        .flatten()
}

/// Guarda la clave en el llavero del sistema. No se devuelve nunca a la interfaz.
#[tauri::command]
async fn set_api_key(state: State<'_, AppState>, provider: String, key: String) -> Result<(), String> {
    let key = key.trim().to_string();
    if key.is_empty() || key.len() > 400 || key.chars().any(char::is_whitespace) {
        return Err("La clave no parece válida. Pégala completa, sin espacios.".into());
    }
    tauri::async_runtime::spawn_blocking(move || key_entry(&provider)?.set_password(&key).map_err(keyring_error))
        .await
        .map_err(|e| e.to_string())??;
    // Los chats abiertos se reconstruyen con la lista de proveedores nueva (su historial queda guardado).
    state.chats.lock().await.clear();
    Ok(())
}

#[tauri::command]
async fn remove_api_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || match key_entry(&provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keyring_error(e)),
    })
    .await
    .map_err(|e| e.to_string())??;
    state.chats.lock().await.clear();
    Ok(())
}

#[derive(Serialize)]
struct ProviderStatus {
    id: &'static str,
    /// Hay un adaptador en CoModel para este proveedor.
    available: bool,
    /// Está listo para usarse (CLI instalado o clave guardada).
    ready: bool,
    detail: String,
}

/// Codex está listo si se encuentra el ejecutable y tiene una sesión iniciada (ChatGPT o clave propia).
async fn codex_status() -> ProviderStatus {
    let Some(bin) = comodel_core::codex::find_binary() else {
        return ProviderStatus { id: "codex", available: true, ready: false, detail: "No se encontró Codex. Instala la app de ChatGPT o el CLI de Codex.".into() };
    };
    let out = tokio::process::Command::new(&bin).args(["login", "status"]).output().await;
    let (ready, detail) = match out {
        Ok(o) if o.status.success() => (true, String::from_utf8_lossy(&o.stdout).trim().to_string()),
        Ok(_) => (false, "Codex no tiene la sesión iniciada. Ejecuta codex login.".to_string()),
        Err(_) => (false, "El comando de Codex no respondió.".to_string()),
    };
    ProviderStatus { id: "codex", available: true, ready, detail }
}

#[derive(Serialize, Clone)]
struct OpenRouterModel {
    id: String,
    name: String,
    /// Admite el parámetro de razonamiento (esfuerzo).
    reasoning: bool,
    context: Option<u64>,
}

/// Modelos que ofrece OpenRouter ahora mismo. La lista es pública; se pide una vez por sesión.
#[tauri::command]
async fn openrouter_models(state: State<'_, AppState>) -> Result<Vec<OpenRouterModel>, String> {
    if let Some(list) = state.openrouter_models.lock().await.clone() {
        return Ok(list);
    }
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let v: serde_json::Value = client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
        .map_err(|_| "No se pudo pedir la lista de modelos de OpenRouter.".to_string())?
        .json()
        .await
        .map_err(|_| "OpenRouter devolvió una lista que no se entiende.".to_string())?;
    let list: Vec<OpenRouterModel> = v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let reasoning = m["supported_parameters"].as_array().is_some_and(|p| p.iter().any(|x| x == "reasoning"));
            Some(OpenRouterModel { name: m["name"].as_str().unwrap_or(&id).to_string(), id, reasoning, context: m["context_length"].as_u64() })
        })
        .collect();
    *state.openrouter_models.lock().await = Some(list.clone());
    Ok(list)
}

#[tauri::command]
async fn chatgpt_accounts() -> Result<Vec<chatgpt_auth::Account>, String> { chatgpt_auth::accounts().await }
#[tauri::command]
async fn chatgpt_sign_in(app: AppHandle, account_id: Option<String>) -> Result<String, String> { chatgpt_auth::sign_in(app, account_id).await }
#[tauri::command]
async fn chatgpt_cancel_sign_in() { chatgpt_auth::cancel(); }
#[tauri::command]
async fn chatgpt_select_account(id: String) -> Result<(), String> { chatgpt_auth::select(id).await }
#[tauri::command]
async fn chatgpt_sign_out() -> Result<String, String> { chatgpt_auth::sign_out().await }
#[tauri::command]
async fn chatgpt_models() -> Result<Vec<chatgpt_auth::Model>, String> { chatgpt_auth::models().await }

/// Modelos y esfuerzos que tu cuenta de Codex ofrece (los mantiene el propio Codex en `~/.codex`).
#[tauri::command]
async fn codex_models() -> comodel_core::codex::CodexCatalog {
    tauri::async_runtime::spawn_blocking(comodel_core::codex::catalog).await.unwrap_or_default()
}

#[tauri::command]
async fn provider_status() -> Vec<ProviderStatus> {
    let claude = match tokio::process::Command::new("claude").arg("--version").output().await {
        Ok(o) if o.status.success() => (true, String::from_utf8_lossy(&o.stdout).trim().to_string()),
        _ => (false, "No se encontró el comando claude. Instala Claude Code.".to_string()),
    };
    let has_key = stored_key("openrouter").await.is_some();
    let gpt = chatgpt_auth::status().await;
    vec![
        ProviderStatus { id: "claude", available: true, ready: claude.0, detail: claude.1 },
        ProviderStatus {
            id: "openrouter",
            available: true,
            ready: has_key,
            detail: if has_key { "Clave guardada".into() } else { "Sin clave".into() },
        },
        codex_status().await,
        ProviderStatus { id: "chatgpt", available: true, ready: gpt.0, detail: gpt.1 },
        ProviderStatus { id: "gemini", available: false, ready: false, detail: "Próximamente".into() },
    ]
}

/// Comprueba la conexión con un proveedor y devuelve un texto para mostrar.
#[tauri::command]
async fn test_connection(provider: String) -> Result<String, String> {
    match provider.as_str() {
        "openrouter" => {
            let key = stored_key("openrouter").await.ok_or("Todavía no hay una clave guardada.")?;
            let info = OpenRouter::new("openrouter", key).key_info().await?;
            Ok(match info.limit {
                Some(limit) => format!("Conexión correcta. Gastado {:.2} de {:.2} dólares.", info.usage, limit),
                None => format!("Conexión correcta. Gastado {:.2} dólares, sin límite en la clave.", info.usage),
            })
        }
        "claude" => {
            let out = tokio::process::Command::new("claude").arg("--version").output().await.map_err(|_| "No se encontró el comando claude.")?;
            if out.status.success() {
                Ok(format!("{} instalado. Usa tu sesión de Claude Code.", String::from_utf8_lossy(&out.stdout).trim()))
            } else {
                Err("El comando claude no respondió correctamente.".into())
            }
        }
        "codex" => {
            let st = codex_status().await;
            if st.ready { Ok(format!("Codex listo. {}.", st.detail.trim_end_matches('.'))) } else { Err(st.detail) }
        }
        _ => Err("Ese proveedor todavía no está disponible.".into()),
    }
}

/// Uso de los últimos `days` días, sumando todos los proyectos y los chats sin carpeta.
#[tauri::command]
async fn usage_summary(state: State<'_, AppState>, days: u32) -> Result<UsageSummary, String> {
    let mut dirs: Vec<PathBuf> = vec![state.general_dir.clone()];
    dirs.extend(state.projects.lock().await.state().projects.iter().map(|p| p.path.clone()));
    tauri::async_runtime::spawn_blocking(move || {
        let mut parts = Vec::new();
        for d in dirs {
            // Solo se lee lo que ya existe: no se crean carpetas .comodel en proyectos que no se han usado.
            if d.join(".comodel").join("memory.db").exists() {
                parts.push(Memory::open(&d, "_resumen").and_then(|m| m.usage_summary(days)).map_err(|e| e.to_string())?);
            }
        }
        Ok(UsageSummary::merge(days.clamp(1, 90), parts))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `/clear`: lo hablado hasta ahora deja de contar como contexto. El historial guardado se conserva.
#[tauri::command]
async fn clear_chat(state: State<'_, AppState>, project_id: Option<String>, chat_id: String) -> Result<(), String> {
    let ws = workspace(&state, project_id.as_deref()).await?;
    if !ws.path.join(".comodel").join("memory.db").exists() {
        return Ok(());
    }
    let orch = chat(&state, project_id.as_deref(), &chat_id).await?;
    let orch = orch.lock().await;
    orch.memory().clear_context().map_err(|e| e.to_string())
}

/// Libera el orquestador de un chat cerrado. El historial queda guardado en la memoria.
#[derive(Serialize)]
struct ImportResult {
    title: String,
    count: usize,
}

/// Guarda un chat importado como historial del chat `chat_id`.
async fn store_imported(state: &AppState, project_id: Option<&str>, chat_id: String, imported: import::Imported) -> Result<ImportResult, String> {
    if !valid_id(&chat_id) {
        return Err("identificador no válido".into());
    }
    let ws = workspace(state, project_id).await?;
    let (title, count) = (imported.title.clone(), imported.messages.len());
    tauri::async_runtime::spawn_blocking(move || {
        use comodel_core::provider::Role;
        let mem = Memory::open(&ws.path, &chat_id).map_err(|e| e.to_string())?;
        for (user, text) in &imported.messages {
            let (role, provider) = if *user { (Role::User, None) } else { (Role::Assistant, Some(imported.source.as_str())) };
            mem.add_message(role, provider, text, false).map_err(|e| e.to_string())?;
        }
        mem.set_chat_title(&imported.title).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(ImportResult { title, count })
}

/// Descarga un chat compartido de ChatGPT o Claude y lo guarda como historial del chat `chat_id`.
#[tauri::command]
async fn import_chat(state: State<'_, AppState>, project_id: Option<String>, chat_id: String, url: String) -> Result<ImportResult, String> {
    let imported = import::import(&url).await?;
    store_imported(&state, project_id.as_deref(), chat_id, imported).await
}

/// Importa una sesión de Claude Code desde su archivo `.jsonl`.
#[tauri::command]
async fn import_chat_file(state: State<'_, AppState>, project_id: Option<String>, chat_id: String, path: String) -> Result<ImportResult, String> {
    let imported = tauri::async_runtime::spawn_blocking(move || import::import_file(&path)).await.map_err(|e| e.to_string())??;
    store_imported(&state, project_id.as_deref(), chat_id, imported).await
}

#[tauri::command]
async fn account_status() -> (bool, Option<account::Account>) {
    (account::configured(), account::current().await)
}

#[tauri::command]
async fn account_sign_in(email: String, password: String) -> Result<account::Account, String> {
    account::sign_in_email(&email, &password).await
}

#[tauri::command]
async fn account_sign_up(email: String, password: String) -> Result<account::SignUp, String> {
    account::sign_up_email(&email, &password).await
}

#[tauri::command]
async fn account_sign_in_oauth(app: AppHandle, provider: String) -> Result<account::Account, String> {
    account::sign_in_oauth(&app, &provider).await
}

#[tauri::command]
async fn account_sign_out() -> Result<(), String> {
    account::sign_out().await
}

#[tauri::command]
async fn close_chat(state: State<'_, AppState>, chat_id: String) -> Result<(), String> {
    state.chats.lock().await.remove(&chat_id);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let store = ProjectStore::open(&app.path().app_config_dir()?)?;
            let general_dir = app.path().app_data_dir()?.join("general");
            std::fs::create_dir_all(&general_dir)?;
            app.manage(AppState {
                projects: Mutex::new(store),
                chats: Mutex::new(HashMap::new()),
                general_dir,
                interactions: Interactions::new(),
                running: Mutex::new(HashMap::new()),
                openrouter_models: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            send_message,
            close_chat,
            import_chat,
            import_chat_file,
            account_status,
            account_sign_in,
            account_sign_up,
            account_sign_in_oauth,
            account_sign_out,
            clear_chat,
            list_dir,
            read_file,
            file_mtime,
            write_file,
            git_status,
            git_diff,
            stop_message,
            answer_interaction,
            chat_history,
            save_pasted_image,
            read_image,
            move_chat_workspace,
            write_document,
            list_projects,
            create_project,
            open_folder,
            create_chat_workspace,
            update_project,
            remove_project,
            set_default_relay,
            set_api_key,
            remove_api_key,
            provider_status,
            openrouter_models,
            codex_models,
            chatgpt_accounts,
            chatgpt_sign_in,
            chatgpt_cancel_sign_in,
            chatgpt_select_account,
            chatgpt_sign_out,
            chatgpt_models,
            test_connection,
            usage_summary
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
