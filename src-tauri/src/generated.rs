//! Archivos que una IA sin acceso a disco entrega en bloques de código: se guardan en la carpeta
//! del workspace o se empaquetan en un zip que elige el usuario.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_FILES: usize = 200;
const MAX_TOTAL: usize = 20 * 1024 * 1024;

#[derive(Deserialize, Clone)]
pub struct GenFile {
    pub path: String,
    pub content: String,
}

#[derive(Serialize, Default)]
pub struct SaveReport {
    pub written: Vec<String>,
    /// Ya existían: no se tocan.
    pub skipped: Vec<String>,
}

/// Ruta relativa y sin sorpresas: nada de `..`, rutas absolutas, `\` ni caracteres de control.
pub fn clean_path(raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 200 || raw.contains('\\') || raw.chars().any(|c| c.is_control()) {
        return Err(format!("Ruta no válida: {raw:?}"));
    }
    let p = Path::new(raw);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::CurDir => {}
            _ => return Err(format!("Ruta no válida (debe ser relativa y sin ..): {raw:?}")),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(format!("Ruta no válida: {raw:?}"));
    }
    Ok(out)
}

fn check(files: &[GenFile]) -> Result<Vec<(PathBuf, &str)>, String> {
    if files.is_empty() {
        return Err("No hay archivos.".into());
    }
    if files.len() > MAX_FILES {
        return Err(format!("Demasiados archivos (máximo {MAX_FILES})."));
    }
    if files.iter().map(|f| f.content.len()).sum::<usize>() > MAX_TOTAL {
        return Err("Los archivos pesan demasiado (máximo 20 MB).".into());
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for f in files {
        let p = clean_path(&f.path)?;
        // Si la IA repite un archivo, vale el último.
        if !seen.insert(p.clone()) {
            out.retain(|(q, _): &(PathBuf, &str)| q != &p);
        }
        out.push((p, f.content.as_str()));
    }
    Ok(out)
}

/// Escribe un zip con los archivos en `dest`.
pub fn write_zip(dest: &Path, files: &[GenFile]) -> Result<usize, String> {
    let list = check(files)?;
    let file = std::fs::File::create(dest).map_err(|e| format!("No se pudo crear el zip: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (path, content) in &list {
        zip.start_file(path.to_string_lossy().replace('\\', "/"), opts).map_err(|e| e.to_string())?;
        zip.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(list.len())
}

/// Guarda los archivos dentro de `root` sin sobrescribir nada que ya exista.
pub fn save_in(root: &Path, files: &[GenFile]) -> Result<SaveReport, String> {
    let list = check(files)?;
    let base = root.canonicalize().map_err(|e| format!("No se pudo abrir la carpeta: {e}"))?;
    let mut report = SaveReport::default();
    for (rel, content) in list {
        let target = base.join(&rel);
        let shown = rel.to_string_lossy().to_string();
        if target.exists() {
            report.skipped.push(shown);
            continue;
        }
        let parent = target.parent().ok_or("Ruta no válida.")?;
        std::fs::create_dir_all(parent).map_err(|e| format!("No se pudo crear {}: {e}", parent.display()))?;
        // Un enlace simbólico dentro de la carpeta no puede sacar el archivo de ella.
        if !parent.canonicalize().map_err(|e| e.to_string())?.starts_with(&base) {
            return Err(format!("{shown} queda fuera de la carpeta del workspace."));
        }
        std::fs::write(&target, content).map_err(|e| format!("No se pudo guardar {shown}: {e}"))?;
        report.written.push(shown);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gf(path: &str, content: &str) -> GenFile {
        GenFile { path: path.into(), content: content.into() }
    }

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("comodel-gen-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn paths_must_be_relative_and_stay_inside() {
        assert!(clean_path("src/app.py").is_ok());
        assert!(clean_path("./a.txt").is_ok());
        for bad in ["", "/etc/passwd", "../x", "a/../../x", "a\\b", "x\0y", "~/../x"] {
            assert!(clean_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_zip_holds_every_file_with_its_content() {
        let dir = tmp("zip");
        let dest = dir.join("out.zip");
        let n = write_zip(&dest, &[gf("a.txt", "hola"), gf("src/b.rs", "fn main() {}"), gf("a.txt", "hola 2")]).unwrap();
        assert_eq!(n, 2, "un archivo repetido cuenta una vez");
        let mut z = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let mut names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
        names.sort();
        assert_eq!(names, ["a.txt", "src/b.rs"]);
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("a.txt").unwrap(), &mut s).unwrap();
        assert_eq!(s, "hola 2", "vale la última versión");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_never_overwrites_and_never_escapes() {
        let dir = tmp("save");
        std::fs::write(dir.join("existe.txt"), "mío").unwrap();
        let r = save_in(&dir, &[gf("existe.txt", "otro"), gf("nuevo/dentro.txt", "ok")]).unwrap();
        assert_eq!(r.written, ["nuevo/dentro.txt"]);
        assert_eq!(r.skipped, ["existe.txt"]);
        assert_eq!(std::fs::read_to_string(dir.join("existe.txt")).unwrap(), "mío");
        assert!(save_in(&dir, &[gf("../fuera.txt", "x")]).is_err());
        // Un enlace dentro de la carpeta que apunta afuera tampoco sirve.
        let out = tmp("save-out");
        std::os::unix::fs::symlink(&out, dir.join("enlace")).unwrap();
        assert!(save_in(&dir, &[gf("enlace/robado.txt", "x")]).is_err());
        assert!(!out.join("robado.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out);
    }
}
