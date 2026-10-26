//! Archivos de un proyecto: listar, leer, guardar y consultar git. Todo está confinado a la carpeta del
//! proyecto: ninguna ruta, con `..` o con enlaces simbólicos, puede salir de ella.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use base64::Engine;
use serde::Serialize;

/// Lo que no se muestra en el árbol: ruido que casi nadie quiere abrir.
const HIDDEN: [&str; 6] = [".git", "node_modules", "target", ".comodel", "__pycache__", ".DS_Store"];
const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_ENTRIES: usize = 3000;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// Texto para mostrar al usuario tal cual.
    #[error("{0}")]
    Msg(String),
    /// El archivo cambió en disco desde que se abrió.
    #[error("conflicto")]
    Conflict,
}

fn msg<T>(m: &str) -> Result<T, FileError> {
    Err(FileError::Msg(m.to_string()))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FileContent {
    Text { content: String, size: u64, mtime: u64 },
    Image { data: String, mime: String, size: u64, mtime: u64 },
    /// No se puede mostrar: binario o demasiado grande.
    Unsupported { reason: String, size: u64, mtime: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GitChange {
    pub path: String,
    /// `M` modificado, `A` nuevo, `?` sin seguimiento, `D` borrado, `R` renombrado.
    pub status: String,
}

fn mtime_ms(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn canonical_root(root: &Path) -> Result<PathBuf, FileError> {
    fs::canonicalize(root).or_else(|_| msg("No se pudo abrir la carpeta del proyecto."))
}

/// Parte relativa limpia: sin raíz, sin `..`, sin bytes nulos.
fn clean_rel(rel: &str) -> Result<PathBuf, FileError> {
    if rel.contains('\0') {
        return msg("Ruta no válida.");
    }
    let p = Path::new(rel);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return msg("La ruta debe quedarse dentro de la carpeta del proyecto."),
        }
    }
    Ok(out)
}

/// Ruta de un archivo o carpeta que ya existe, comprobando que (tras seguir enlaces) sigue dentro del proyecto.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf, FileError> {
    let root = canonical_root(root)?;
    let full = fs::canonicalize(root.join(clean_rel(rel)?)).or_else(|_| msg("No se encontró el archivo."))?;
    if !full.starts_with(&root) {
        return msg("Ese archivo está fuera de la carpeta del proyecto.");
    }
    Ok(full)
}

/// Ruta de un archivo que puede no existir todavía: su carpeta sí debe existir y estar dentro del proyecto.
fn resolve_for_write(root: &Path, rel: &str) -> Result<PathBuf, FileError> {
    let root = canonical_root(root)?;
    let rel = clean_rel(rel)?;
    let Some(name) = rel.file_name().map(|n| n.to_owned()) else {
        return msg("Falta el nombre del archivo.");
    };
    let parent = fs::canonicalize(root.join(rel.parent().unwrap_or(Path::new("")))).or_else(|_| msg("La carpeta del archivo no existe."))?;
    if !parent.starts_with(&root) {
        return msg("Ese archivo está fuera de la carpeta del proyecto.");
    }
    let full = parent.join(name);
    // Si ya existe como enlace, se resuelve para no escribir fuera a través de él.
    if full.exists() {
        let real = fs::canonicalize(&full).or_else(|_| msg("No se pudo abrir el archivo."))?;
        if !real.starts_with(&root) {
            return msg("Ese archivo está fuera de la carpeta del proyecto.");
        }
        return Ok(real);
    }
    Ok(full)
}

/// Contenido de una carpeta: primero las carpetas, luego los archivos, por nombre.
pub fn list_dir(root: &Path, rel: &str) -> Result<Vec<Entry>, FileError> {
    let dir = resolve(root, rel)?;
    if !dir.is_dir() {
        return msg("Eso no es una carpeta.");
    }
    let mut out = Vec::new();
    let rd = fs::read_dir(&dir).or_else(|_| msg("No se pudo leer la carpeta."))?;
    for e in rd.flatten().take(MAX_ENTRIES) {
        let name = e.file_name().to_string_lossy().to_string();
        if HIDDEN.contains(&name.as_str()) {
            continue;
        }
        // Un enlace que apunta fuera del proyecto se muestra, pero no se podrá abrir (resolve lo impide).
        let meta = fs::metadata(e.path()).or_else(|_| e.metadata());
        let Ok(meta) = meta else { continue };
        out.push(Entry { name, is_dir: meta.is_dir(), size: if meta.is_dir() { 0 } else { meta.len() } });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

fn image_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

pub fn read_file(root: &Path, rel: &str) -> Result<FileContent, FileError> {
    let path = resolve(root, rel)?;
    let meta = fs::metadata(&path).or_else(|_| msg("No se pudo leer el archivo."))?;
    if !meta.is_file() {
        return msg("Eso no es un archivo.");
    }
    let (size, mtime) = (meta.len(), mtime_ms(&meta));
    if let Some(mime) = image_mime(&path) {
        if size > MAX_IMAGE_BYTES {
            return Ok(FileContent::Unsupported { reason: "La imagen pesa más de 5 MB.".into(), size, mtime });
        }
        let bytes = fs::read(&path).or_else(|_| msg("No se pudo leer el archivo."))?;
        return Ok(FileContent::Image { data: base64::engine::general_purpose::STANDARD.encode(bytes), mime: mime.into(), size, mtime });
    }
    if size > MAX_TEXT_BYTES {
        return Ok(FileContent::Unsupported { reason: "El archivo pesa más de 2 MB.".into(), size, mtime });
    }
    let bytes = fs::read(&path).or_else(|_| msg("No se pudo leer el archivo."))?;
    if bytes.contains(&0) {
        return Ok(FileContent::Unsupported { reason: "No es un archivo de texto.".into(), size, mtime });
    }
    match String::from_utf8(bytes) {
        Ok(content) => Ok(FileContent::Text { content, size, mtime }),
        Err(_) => Ok(FileContent::Unsupported { reason: "No es texto en UTF-8.".into(), size, mtime }),
    }
}

pub fn mtime(root: &Path, rel: &str) -> Result<u64, FileError> {
    let path = resolve(root, rel)?;
    Ok(mtime_ms(&fs::metadata(path).or_else(|_| msg("No se encontró el archivo."))?))
}

/// Guarda el archivo. Con `expected_mtime`, si en disco cambió desde que se abrió devuelve `Conflict`
/// (así no se pisa lo que escribió otra IA u otro programa). Devuelve la nueva fecha de modificación.
pub fn write_file(root: &Path, rel: &str, content: &str, expected_mtime: Option<u64>) -> Result<u64, FileError> {
    if content.len() as u64 > MAX_TEXT_BYTES * 4 {
        return msg("El contenido es demasiado grande.");
    }
    let path = resolve_for_write(root, rel)?;
    if let (Some(expected), Ok(meta)) = (expected_mtime, fs::metadata(&path)) {
        if mtime_ms(&meta) != expected {
            return Err(FileError::Conflict);
        }
    }
    fs::write(&path, content).or_else(|_| msg("No se pudo guardar el archivo."))?;
    Ok(mtime_ms(&fs::metadata(&path).or_else(|_| msg("No se pudo leer el archivo guardado."))?))
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// Archivos con cambios respecto al último commit. Vacío si la carpeta no es un repositorio git.
pub fn git_status(root: &Path) -> Vec<GitChange> {
    let Ok(root) = canonical_root(root) else { return Vec::new() };
    let Some(text) = git(&root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut parts = text.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let (code, path) = (&entry[..2], &entry[3..]);
        let status = match code {
            "??" => "?",
            c if c.contains('D') => "D",
            c if c.contains('R') => {
                parts.next(); // el nombre anterior viene en la entrada siguiente
                "R"
            }
            c if c.contains('A') => "A",
            _ => "M",
        };
        out.push(GitChange { path: path.to_string(), status: status.into() });
    }
    out
}

/// `git diff` de un archivo contra el último commit. `None` si no es un repositorio.
pub fn git_diff(root: &Path, rel: &str) -> Option<String> {
    let root = canonical_root(root).ok()?;
    let rel = clean_rel(rel).ok()?;
    git(&root, &["diff", "HEAD", "--no-color", "--no-ext-diff", "--", &rel.to_string_lossy()])
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("comodel-files-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn project(t: &Tmp) -> PathBuf {
        let p = t.0.join("proj");
        fs::create_dir_all(p.join("src")).unwrap();
        fs::create_dir_all(p.join("node_modules/x")).unwrap();
        fs::create_dir_all(p.join(".git")).unwrap();
        fs::write(p.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(p.join("README.md"), "# hola\n").unwrap();
        p
    }

    #[test]
    fn lists_folders_first_and_hides_noise() {
        let t = Tmp::new("list");
        let p = project(&t);
        let names: Vec<_> = list_dir(&p, "").unwrap().into_iter().map(|e| (e.name, e.is_dir)).collect();
        assert_eq!(names, vec![("src".to_string(), true), ("README.md".to_string(), false)]);
        let inner: Vec<_> = list_dir(&p, "src").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(inner, vec!["main.rs".to_string()]);
    }

    #[test]
    fn nothing_can_escape_the_project_folder() {
        let t = Tmp::new("escape");
        let p = project(&t);
        let secret = t.0.join("secreto.txt");
        fs::write(&secret, "no se debe leer").unwrap();

        for bad in ["../secreto.txt", "src/../../secreto.txt", "/etc/passwd", "src/../.."] {
            assert!(read_file(&p, bad).is_err(), "{bad} debería rechazarse");
            assert!(write_file(&p, bad, "x", None).is_err(), "{bad} no debería poder escribirse");
        }
        // Un enlace simbólico que apunta fuera tampoco sirve, ni para leer ni para escribir ni para listar.
        std::os::unix::fs::symlink(&secret, p.join("enlace.txt")).unwrap();
        std::os::unix::fs::symlink(&t.0, p.join("carpeta-fuera")).unwrap();
        assert!(read_file(&p, "enlace.txt").is_err());
        assert!(write_file(&p, "enlace.txt", "pisado", None).is_err());
        assert!(list_dir(&p, "carpeta-fuera").is_err());
        assert_eq!(fs::read_to_string(&secret).unwrap(), "no se debe leer");
    }

    #[test]
    fn reads_text_images_and_refuses_binaries() {
        let t = Tmp::new("read");
        let p = project(&t);
        match read_file(&p, "src/main.rs").unwrap() {
            FileContent::Text { content, .. } => assert_eq!(content, "fn main() {}\n"),
            other => panic!("{other:?}"),
        }
        fs::write(p.join("datos.bin"), [0u8, 1, 2, 255]).unwrap();
        assert!(matches!(read_file(&p, "datos.bin").unwrap(), FileContent::Unsupported { .. }));
        fs::write(p.join("foto.png"), [137u8, 80, 78, 71]).unwrap();
        assert!(matches!(read_file(&p, "foto.png").unwrap(), FileContent::Image { .. }));
        fs::write(p.join("latin1.txt"), [0x68u8, 0xe9]).unwrap();
        assert!(matches!(read_file(&p, "latin1.txt").unwrap(), FileContent::Unsupported { .. }));
        assert!(read_file(&p, "no-existe.txt").is_err());
    }

    #[test]
    fn saving_detects_changes_made_meanwhile() {
        let t = Tmp::new("write");
        let p = project(&t);
        let FileContent::Text { mtime: m0, .. } = read_file(&p, "README.md").unwrap() else { panic!() };

        // Guardar con la fecha correcta funciona y devuelve la nueva.
        let m1 = write_file(&p, "README.md", "# nuevo\n", Some(m0)).unwrap();
        assert_eq!(fs::read_to_string(p.join("README.md")).unwrap(), "# nuevo\n");

        // Otra IA lo cambió mientras tanto: guardar con la fecha vieja se rechaza y no pisa nada.
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(p.join("README.md"), "# de otra IA\n").unwrap();
        assert!(matches!(write_file(&p, "README.md", "# mío\n", Some(m1)), Err(FileError::Conflict)));
        assert_eq!(fs::read_to_string(p.join("README.md")).unwrap(), "# de otra IA\n");
        // Sin fecha esperada (el usuario eligió sobrescribir) sí guarda.
        assert!(write_file(&p, "README.md", "# mío\n", None).is_ok());

        // Archivos nuevos dentro de una carpeta que existe.
        assert!(write_file(&p, "src/nuevo.rs", "// nuevo\n", None).is_ok());
        assert!(write_file(&p, "no-existe/a.rs", "x", None).is_err());
    }

    #[test]
    fn reports_git_changes_and_diffs() {
        let t = Tmp::new("git");
        let p = project(&t);
        let run = |args: &[&str]| {
            Command::new("git").arg("-C").arg(&p).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap()
        };
        fs::remove_dir_all(p.join(".git")).unwrap();
        // Sin repositorio: nada que informar.
        assert!(git_status(&p).is_empty());
        assert!(git_diff(&p, "README.md").is_none());

        run(&["init", "-q"]);
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "inicio"]);
        fs::write(p.join("README.md"), "# cambiado\n").unwrap();
        fs::write(p.join("nuevo.txt"), "hola\n").unwrap();

        let status: std::collections::HashMap<_, _> = git_status(&p).into_iter().map(|c| (c.path, c.status)).collect();
        assert_eq!(status.get("README.md").map(String::as_str), Some("M"));
        assert_eq!(status.get("nuevo.txt").map(String::as_str), Some("?"));

        let diff = git_diff(&p, "README.md").unwrap();
        assert!(diff.contains("-# hola") && diff.contains("+# cambiado"), "{diff}");
    }
}
