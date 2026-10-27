//! Memoria que el usuario trae de otras IAs (lo que ChatGPT, Gemini… recuerdan de él). Se guarda en
//! `~/.ai-memory/imports/` y `ai-memory-sync` la une con la de Claude y Codex.

use std::path::{Path, PathBuf};

use serde::Serialize;

const MAX_CHARS: usize = 30_000;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ImportInfo {
    pub id: String,
    pub label: String,
    pub chars: usize,
    /// Segundos desde 1970 de la última vez que se guardó.
    pub updated: u64,
}

pub fn root() -> Result<PathBuf, String> {
    Ok(PathBuf::from(std::env::var_os("HOME").ok_or("No se encontró la carpeta personal.")?).join(".ai-memory"))
}

fn dir_of(root: &Path) -> PathBuf {
    root.join("imports")
}

/// Nombre de archivo seguro a partir de lo que escribió el usuario ("Otra IA" -> "otra-ia").
pub fn slug(label: &str) -> Option<String> {
    let s: String = label
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().collect::<String>() } else { "-".into() })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let s: String = s.chars().take(30).collect();
    (!s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '-')).then_some(s)
}

fn label_of(id: &str) -> String {
    match id {
        "chatgpt" => "ChatGPT".into(),
        "gemini" => "Gemini".into(),
        "grok" => "Grok".into(),
        other => {
            let mut c = other.replace('-', " ").chars().collect::<Vec<_>>();
            if let Some(f) = c.first_mut() {
                *f = f.to_uppercase().next().unwrap_or(*f);
            }
            c.into_iter().collect()
        }
    }
}

/// Texto que parece contener una clave o un secreto: se rechaza, porque acabaría en todos los chats.
pub fn looks_secret(text: &str) -> Option<&'static str> {
    let t = text;
    let has_run = |prefix: &str, min: usize| {
        t.match_indices(prefix).any(|(i, _)| t[i + prefix.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').count() >= min)
    };
    if has_run("sk-", 20) || has_run("sk-ant-", 20) || has_run("ghp_", 30) || has_run("gho_", 30) || has_run("github_pat_", 30) || has_run("AKIA", 16) || has_run("sb_secret_", 20) || has_run("xoxb-", 20) {
        return Some("una clave de API");
    }
    if t.contains("-----BEGIN") && t.contains("PRIVATE KEY") {
        return Some("una clave privada");
    }
    None
}

pub fn list_in(root: &Path) -> Vec<ImportInfo> {
    let mut out: Vec<ImportInfo> = std::fs::read_dir(dir_of(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()? != "md" {
                return None;
            }
            let id = path.file_stem()?.to_string_lossy().to_string();
            let meta = e.metadata().ok()?;
            let updated = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            let chars = std::fs::read_to_string(&path).ok()?.chars().count();
            Some(ImportInfo { label: label_of(&id), id, chars, updated })
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

fn path_of(root: &Path, id: &str) -> Result<PathBuf, String> {
    // El id llega de la interfaz: solo letras, números y guiones.
    if id.is_empty() || !id.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err("Fuente no válida.".into());
    }
    Ok(dir_of(root).join(format!("{id}.md")))
}

pub fn get_in(root: &Path, id: &str) -> Result<String, String> {
    std::fs::read_to_string(path_of(root, id)?).map_err(|_| "No se encontró esa memoria.".to_string())
}

pub fn save_in(root: &Path, label: &str, text: &str) -> Result<ImportInfo, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Pega primero lo que te dio la IA.".into());
    }
    if text.chars().count() > MAX_CHARS {
        return Err(format!("Es demasiado largo ({} caracteres como máximo). Pídele a la IA una versión más corta.", MAX_CHARS));
    }
    if let Some(what) = looks_secret(text) {
        return Err(format!("El texto parece incluir {what}. Quítala antes de importarlo: se compartiría con todas las IAs."));
    }
    let id = slug(label).ok_or("Ponle un nombre a la fuente (por ejemplo ChatGPT).")?;
    let dir = dir_of(root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo preparar la carpeta de memoria: {e}"))?;
    std::fs::write(path_of(root, &id)?, format!("{text}\n")).map_err(|e| format!("No se pudo guardar: {e}"))?;
    list_in(root).into_iter().find(|i| i.id == id).ok_or_else(|| "No se pudo guardar.".to_string())
}

pub fn delete_in(root: &Path, id: &str) -> Result<(), String> {
    match std::fs::remove_file(path_of(root, id)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("No se pudo quitar: {e}")),
    }
}

/// Dónde está `ai-memory-sync`: en el PATH o en `~/.local/bin`.
pub fn find_sync() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .chain(home.map(|h| h.join(".local/bin")))
        .map(|d| d.join("ai-memory-sync"))
        .find(|p| p.is_file())
}

#[derive(Serialize)]
pub struct SyncReport {
    /// Se encontró el programa que junta la memoria.
    pub available: bool,
    pub ok: bool,
    pub message: String,
}

pub fn run_sync() -> SyncReport {
    let Some(bin) = find_sync() else {
        return SyncReport { available: false, ok: false, message: "No se encontró `ai-memory-sync`. La memoria se guardó, pero Claude y Codex no la leerán hasta instalarlo.".into() };
    };
    match std::process::Command::new(bin).output() {
        Ok(o) if o.status.success() => SyncReport { available: true, ok: true, message: String::from_utf8_lossy(&o.stdout).trim().to_string() },
        Ok(o) => SyncReport { available: true, ok: false, message: String::from_utf8_lossy(&o.stderr).trim().chars().take(300).collect() },
        Err(e) => SyncReport { available: true, ok: false, message: format!("No se pudo ejecutar: {e}") },
    }
}

/// Tamaño del texto compartido que leen las IAs, si existe.
pub fn shared_chars(root: &Path) -> Option<usize> {
    std::fs::read_to_string(root.join("shared.md")).ok().map(|t| t.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("comodel-mem-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn names_become_safe_file_names() {
        assert_eq!(slug("ChatGPT").as_deref(), Some("chatgpt"));
        assert_eq!(slug("  Otra IA  ").as_deref(), Some("otra-ia"));
        assert_eq!(slug("../../etc/passwd").as_deref(), Some("etc-passwd"));
        assert_eq!(slug("///"), None);
        assert_eq!(slug(""), None);
    }

    #[test]
    fn an_import_is_saved_listed_replaced_and_removed() {
        let root = tmp("flow");
        let a = save_in(&root, "ChatGPT", "QUIÉN SOY\n- Ury\n").unwrap();
        assert_eq!((a.id.as_str(), a.label.as_str()), ("chatgpt", "ChatGPT"));
        assert_eq!(get_in(&root, "chatgpt").unwrap().trim(), "QUIÉN SOY\n- Ury");
        // Importar de nuevo reemplaza, no suma.
        save_in(&root, "chatgpt", "nuevo").unwrap();
        assert_eq!(list_in(&root).len(), 1);
        assert_eq!(get_in(&root, "chatgpt").unwrap().trim(), "nuevo");
        save_in(&root, "Gemini", "otro").unwrap();
        assert_eq!(list_in(&root).iter().map(|i| i.label.clone()).collect::<Vec<_>>(), ["ChatGPT", "Gemini"]);
        delete_in(&root, "chatgpt").unwrap();
        assert_eq!(list_in(&root).len(), 1);
        delete_in(&root, "chatgpt").unwrap(); // quitar lo que no está no falla
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_input_is_refused_before_it_reaches_every_chat() {
        let root = tmp("bad");
        assert!(save_in(&root, "ChatGPT", "   ").is_err());
        assert!(save_in(&root, "", "algo").is_err());
        assert!(save_in(&root, "ChatGPT", &"x".repeat(31_000)).is_err());
        assert!(save_in(&root, "ChatGPT", "mi clave es sk-abcdefghijklmnopqrstuvwxyz123456").is_err());
        assert!(save_in(&root, "ChatGPT", "token ghp_abcdefghijklmnopqrstuvwxyz0123456789").is_err());
        assert!(save_in(&root, "ChatGPT", "-----BEGIN RSA PRIVATE KEY-----\nabc").is_err());
        // Texto normal con "sk-" corto o palabras parecidas pasa.
        assert!(save_in(&root, "ChatGPT", "Me gusta el sk-8 y el hielo").is_ok());
        // El id que llega de la interfaz no puede salirse de la carpeta.
        assert!(get_in(&root, "../shared").is_err());
        assert!(delete_in(&root, "../../x").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
