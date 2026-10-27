//! Archivos que una IA entrega en un mensaje: los que envía directamente y los que escribe en el workspace.
//!
//! Todo lo que llega a la interfaz pasa por aquí: las rutas se validan contra la carpeta de trabajo,
//! nunca se aceptan las que apuntan fuera de ella y hay un límite de tamaño.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Tamaño máximo de un archivo entregado por una IA.
pub const MAX_OUTPUT_BYTES: u64 = 50 * 1024 * 1024;
/// Archivos por mensaje; el resto se descarta.
pub const MAX_OUTPUTS: usize = 8;
/// Las imágenes más grandes que esto se ofrecen como archivo, porque la vista previa no las carga.
const MAX_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;
/// Entradas que se miran al comparar el workspace antes y después de un turno.
const MAX_SCAN_ENTRIES: usize = 20_000;
const MAX_SCAN_DEPTH: usize = 8;
/// Carpetas que no se miran: dependencias y resultados de compilación, no lo que la IA entrega.
const SKIP_DIRS: &[&str] = &[".git", ".comodel", "node_modules", "target", "dist", "build", ".svelte-kit", ".venv", "__pycache__"];

/// Un archivo entregado en el mensaje de una IA. `path` es relativa a la carpeta de trabajo y usa `/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutFile {
    pub path: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// `image` si es una imagen que se puede mostrar; `file` para cualquier otra cosa.
    pub kind: String,
    /// `provider` si la IA lo envió directamente; `workspace` si lo escribió en la carpeta de trabajo.
    pub origin: String,
}

/// Nombre de archivo seguro: sin rutas, sin caracteres raros y sin empezar por punto.
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("archivo");
    let cleaned: String = base.chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let cleaned = cleaned.trim_start_matches('.').chars().take(80).collect::<String>();
    if cleaned.is_empty() { "archivo".into() } else { cleaned }
}

/// Tipo por el contenido para las imágenes (lo que declara la IA no se cree) y por la extensión para el resto.
pub fn mime_for(name: &str, head: &[u8]) -> String {
    let sniffed = if head.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF8") {
        Some("image/gif")
    } else if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else if head.starts_with(b"%PDF") {
        Some("application/pdf")
    } else if head.starts_with(&[b'P', b'K', 3, 4]) {
        Some("application/zip")
    } else {
        None
    };
    if let Some(m) = sniffed {
        return m.into();
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "zip" => "application/zip",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "7z" => "application/x-7z-compressed",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "csv" => "text/csv",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
    .into()
}

/// Solo las imágenes que la vista previa sabe mostrar (no SVG: puede llevar scripts).
fn previewable(mime: &str, size: u64) -> bool {
    matches!(mime, "image/png" | "image/jpeg" | "image/gif" | "image/webp") && size <= MAX_PREVIEW_BYTES
}

fn out_file(rel: String, name: String, mime: String, size: u64, origin: &str) -> OutFile {
    let kind = if previewable(&mime, size) { "image" } else { "file" };
    OutFile { path: rel, name, mime, size, kind: kind.into(), origin: origin.into() }
}

/// `true` si el archivo parece texto: sin bytes nulos y UTF-8 válido al principio.
pub fn looks_like_text(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = fs::File::open(path) else { return false };
    let mut buf = [0u8; 8192];
    let Ok(n) = f.read(&mut buf) else { return false };
    let head = &buf[..n];
    if head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        // Un carácter cortado justo en el límite de lo leído no cuenta como binario.
        Err(e) => e.error_len().is_none() && n == buf.len(),
    }
}

/// Una ruta dentro de la carpeta de trabajo: devuelve la absoluta y la relativa (con `/`).
/// Rechaza lo que sale de la carpeta (también por enlaces simbólicos), las carpetas, `.git` y lo interno de
/// `.comodel/` salvo `.comodel/attachments/`.
pub fn resolve_in_root(root: &Path, path: &str) -> Result<(PathBuf, String), String> {
    let root = fs::canonicalize(root).map_err(|_| "No se pudo abrir la carpeta de trabajo.".to_string())?;
    let p = Path::new(path);
    let joined = if p.is_absolute() { p.to_path_buf() } else { root.join(p) };
    let full = fs::canonicalize(&joined).map_err(|_| "El archivo no existe.".to_string())?;
    let rel = full.strip_prefix(&root).map_err(|_| "El archivo está fuera de la carpeta de trabajo.".to_string())?;
    if !full.is_file() {
        return Err("Solo se pueden entregar archivos, no carpetas.".into());
    }
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    let internal = match parts.first().map(String::as_str) {
        Some(".git") => true,
        Some(".comodel") => parts.get(1).map(String::as_str) != Some("attachments"),
        _ => false,
    };
    if internal {
        return Err("Ese archivo es interno de CoModel o de Git.".into());
    }
    Ok((full, parts.join("/")))
}

/// Describe un archivo ya existente dentro de la carpeta de trabajo.
pub fn describe(root: &Path, path: &str, origin: &str) -> Result<OutFile, String> {
    use std::io::Read;
    let (full, rel) = resolve_in_root(root, path)?;
    let size = fs::metadata(&full).map_err(|_| "No se pudo leer el archivo.".to_string())?.len();
    if size > MAX_OUTPUT_BYTES {
        return Err(format!("pesa más de {} MB", MAX_OUTPUT_BYTES / 1024 / 1024));
    }
    let mut head = [0u8; 16];
    let n = fs::File::open(&full).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    let name = full.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "archivo".into());
    let mime = mime_for(&name, &head[..n]);
    Ok(out_file(rel, name, mime, size, origin))
}

/// Guarda el contenido (base64) que una IA envía directamente en `.comodel/attachments/` del workspace.
pub fn store_data(root: &Path, name: &str, data: &str) -> Result<OutFile, String> {
    // Se mira el tamaño antes de decodificar: cada 4 caracteres base64 son 3 bytes.
    if (data.len() as u64 / 4) * 3 > MAX_OUTPUT_BYTES + 3 {
        return Err(format!("pesa más de {} MB", MAX_OUTPUT_BYTES / 1024 / 1024));
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|_| "el contenido no es válido".to_string())?;
    if bytes.is_empty() {
        return Err("está vacío".into());
    }
    if bytes.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(format!("pesa más de {} MB", MAX_OUTPUT_BYTES / 1024 / 1024));
    }
    let name = safe_name(name);
    let dir = root.join(".comodel").join("attachments");
    fs::create_dir_all(&dir).map_err(|_| "no se pudo preparar la carpeta de adjuntos".to_string())?;
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let stored = format!("{stamp}-{name}");
    fs::write(dir.join(&stored), &bytes).map_err(|_| "no se pudo guardar".to_string())?;
    let mime = mime_for(&name, &bytes[..bytes.len().min(16)]);
    Ok(out_file(format!(".comodel/attachments/{stored}"), name, mime, bytes.len() as u64, "provider"))
}

/// Estado del workspace en un momento: ruta → (fecha de modificación en ns, tamaño).
#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    files: HashMap<PathBuf, (u128, u64)>,
    /// Se llegó al límite de entradas: lo que quedó fuera no se compara.
    pub truncated: bool,
}

impl Snapshot {
    pub fn take(root: &Path) -> Snapshot {
        let mut snap = Snapshot::default();
        let Ok(root) = fs::canonicalize(root) else { return snap };
        let mut stack = vec![(root, 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if snap.files.len() >= MAX_SCAN_ENTRIES {
                    snap.truncated = true;
                    return snap;
                }
                let Ok(ft) = e.file_type() else { continue };
                // Los enlaces simbólicos no se siguen: podrían llevar fuera del workspace.
                if ft.is_symlink() {
                    continue;
                }
                let path = e.path();
                if ft.is_dir() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if depth < MAX_SCAN_DEPTH && !SKIP_DIRS.contains(&name.as_str()) {
                        stack.push((path, depth + 1));
                    }
                } else if ft.is_file() {
                    let Ok(m) = e.metadata() else { continue };
                    let mtime = m.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
                    snap.files.insert(path, (mtime, m.len()));
                }
            }
        }
        snap
    }

    /// Los archivos que no estaban o cambiaron respecto de `before`, ordenados por ruta.
    pub fn changed_since(&self, before: &Snapshot) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.files.iter().filter(|(p, stat)| before.files.get(*p) != Some(*stat)).map(|(p, _)| p.clone()).collect();
        v.sort();
        v
    }

    fn unchanged(&self, path: &Path) -> bool {
        match (self.files.get(path), fs::metadata(path).ok()) {
            (Some(&(mtime, size)), Some(m)) => {
                let now = m.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
                mtime == now && size == m.len()
            }
            _ => false,
        }
    }
}

/// Los archivos que la IA escribió en el workspace durante el turno y que no son texto.
///
/// Se parte de lo que cambió en disco (la foto de antes contra la de ahora, que incluye lo que escribe el
/// shell) y de lo que las herramientas de edición declararon (`touched`), siempre que de verdad haya
/// cambiado. Los de texto se omiten: ya salen en el chat con su diff. Nunca se busca en el texto de la respuesta.
pub fn workspace_outputs(root: &Path, before: &Snapshot, touched: &[String]) -> (Vec<OutFile>, Vec<String>) {
    let after = Snapshot::take(root);
    let mut candidates: Vec<PathBuf> = after.changed_since(before);
    if let Ok(canon_root) = fs::canonicalize(root) {
        for t in touched {
            let p = Path::new(t);
            let full = if p.is_absolute() { p.to_path_buf() } else { canon_root.join(p) };
            if let Ok(full) = fs::canonicalize(full) {
                if !candidates.contains(&full) && !before.unchanged(&full) {
                    candidates.push(full);
                }
            }
        }
    }
    let mut files = Vec::new();
    let mut rejected = Vec::new();
    for full in candidates {
        if files.len() >= MAX_OUTPUTS {
            rejected.push(format!("Hay más de {MAX_OUTPUTS} archivos nuevos; se muestran los primeros."));
            break;
        }
        match describe(root, &full.to_string_lossy(), "workspace") {
            Ok(f) => {
                if !looks_like_text(&full) {
                    files.push(f);
                }
            }
            Err(e) => {
                // Lo interno o lo que no es un archivo simplemente no cuenta; lo demasiado grande se avisa.
                if e.starts_with("pesa") {
                    rejected.push(format!("{}: {e}", full.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                }
            }
        }
    }
    (files, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("comodel-outputs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const ZIP: [u8; 8] = [b'P', b'K', 3, 4, 0, 0, 0, 0];

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn direct_image_is_stored_and_previewable_even_if_the_name_lies() {
        let root = tmp("img");
        let f = store_data(&root, "../../grafica.txt", &b64(&PNG)).unwrap();
        assert_eq!(f.kind, "image");
        assert_eq!(f.mime, "image/png");
        assert_eq!(f.origin, "provider");
        assert!(f.path.starts_with(".comodel/attachments/") && f.path.ends_with("-grafica.txt"), "{}", f.path);
        assert!(root.join(&f.path).is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn direct_binary_is_a_file_not_an_image() {
        let root = tmp("zip");
        let f = store_data(&root, "proyecto.zip", &b64(&ZIP)).unwrap();
        assert_eq!((f.kind.as_str(), f.mime.as_str(), f.size), ("file", "application/zip", 8));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn direct_data_that_is_invalid_empty_or_too_big_is_rejected() {
        let root = tmp("bad");
        assert!(store_data(&root, "a.bin", "no es base64 ***").is_err());
        assert!(store_data(&root, "a.bin", "").is_err());
        let huge = "A".repeat((MAX_OUTPUT_BYTES as usize / 3 + 10) * 4);
        assert!(store_data(&root, "a.bin", &huge).unwrap_err().starts_with("pesa"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn paths_outside_the_workspace_or_internal_are_refused() {
        let root = tmp("paths");
        let outside = tmp("paths-outside");
        fs::write(outside.join("secreto.txt"), "x").unwrap();
        fs::write(root.join("ok.zip"), ZIP).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "x").unwrap();
        fs::create_dir_all(root.join(".comodel")).unwrap();
        fs::write(root.join(".comodel/memory.db"), "x").unwrap();

        assert!(describe(&root, "ok.zip", "workspace").is_ok());
        assert!(describe(&root, &root.join("ok.zip").to_string_lossy(), "workspace").is_ok());
        assert!(describe(&root, &outside.join("secreto.txt").to_string_lossy(), "workspace").unwrap_err().contains("fuera"));
        assert!(describe(&root, "../paths-outside/secreto.txt", "workspace").is_err());
        assert!(describe(&root, ".git/config", "workspace").is_err());
        assert!(describe(&root, ".comodel/memory.db", "workspace").is_err());
        assert!(describe(&root, "no-existe.zip", "workspace").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("secreto.txt"), root.join("enlace.txt")).unwrap();
            assert!(describe(&root, "enlace.txt", "workspace").is_err(), "un enlace no puede sacar archivos de la carpeta");
        }
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn workspace_writes_are_detected_and_text_is_left_to_the_diff() {
        let root = tmp("ws");
        fs::write(root.join("viejo.zip"), ZIP).unwrap();
        fs::write(root.join("notas.md"), "hola").unwrap();
        let before = Snapshot::take(&root);

        fs::write(root.join("entrega.zip"), ZIP).unwrap(); // lo escribe el shell, sin evento
        fs::create_dir_all(root.join("img")).unwrap();
        fs::write(root.join("img/grafica.png"), PNG).unwrap();
        fs::write(root.join("nuevo.md"), "texto").unwrap(); // de texto: ya sale con su diff
        fs::create_dir_all(root.join("node_modules/x")).unwrap();
        fs::write(root.join("node_modules/x/a.bin"), ZIP).unwrap(); // dependencias: no cuentan

        let (files, rejected) = workspace_outputs(&root, &before, &["nuevo.md".into(), "viejo.zip".into()]);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["entrega.zip", "img/grafica.png"], "{files:?}");
        assert!(rejected.is_empty());
        assert_eq!(files[1].kind, "image");
        assert!(files.iter().all(|f| f.origin == "workspace"));
        // `viejo.zip` figuraba como tocado pero no cambió en disco: no es una escritura real.
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn touched_files_outside_the_workspace_are_ignored_and_the_count_is_capped() {
        let root = tmp("cap");
        let outside = tmp("cap-outside");
        fs::write(outside.join("fuera.zip"), ZIP).unwrap();
        let before = Snapshot::take(&root);
        for i in 0..(MAX_OUTPUTS + 3) {
            fs::write(root.join(format!("f{i:02}.zip")), ZIP).unwrap();
        }
        let (files, rejected) = workspace_outputs(&root, &before, &[outside.join("fuera.zip").to_string_lossy().to_string()]);
        assert_eq!(files.len(), MAX_OUTPUTS);
        assert!(files.iter().all(|f| !f.path.contains("fuera")));
        assert_eq!(rejected.len(), 1);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }
}
