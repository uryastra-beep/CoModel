//! Proyectos: una carpeta real del usuario, las IAs que usa y su orden de relevo.
//! Se guardan en un JSON en la carpeta de configuración de la app; los archivos del usuario no se tocan.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// IAs que CoModel sabe usar hoy, en el orden de relevo por defecto.
pub const KNOWN_PROVIDERS: [&str; 4] = ["claude", "openrouter", "codex", "chatgpt"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    /// Ids de las IAs en orden de relevo: si la primera se agota, sigue la segunda.
    pub relay: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectsFile {
    pub projects: Vec<Project>,
    /// Orden de relevo de los chats que no tienen carpeta.
    #[serde(default = "default_relay")]
    pub default_relay: Vec<String>,
}

fn default_relay() -> Vec<String> {
    vec!["claude".to_string()]
}

impl Default for ProjectsFile {
    fn default() -> Self {
        Self { projects: Vec::new(), default_relay: default_relay() }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// Texto pensado para mostrarse tal cual al usuario.
    #[error("{0}")]
    Invalid(String),
    #[error("no se pudo guardar la configuración: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuración dañada: {0}")]
    Json(#[from] serde_json::Error),
}

fn invalid<T>(msg: &str) -> Result<T, ProjectError> {
    Err(ProjectError::Invalid(msg.to_string()))
}

pub struct ProjectStore {
    file: PathBuf,
    data: ProjectsFile,
}

impl ProjectStore {
    /// Abre (o crea) la lista en `config_dir/projects.json`. Si el archivo está dañado se aparta como
    /// `projects.json.dañado` y se empieza con la lista vacía, para no perder nada sin avisar.
    pub fn open(config_dir: &Path) -> Result<Self, ProjectError> {
        fs::create_dir_all(config_dir)?;
        let file = config_dir.join("projects.json");
        let data = match fs::read_to_string(&file) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(d) => d,
                Err(_) => {
                    let _ = fs::rename(&file, config_dir.join("projects.json.dañado"));
                    ProjectsFile::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ProjectsFile::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { file, data })
    }

    pub fn state(&self) -> &ProjectsFile {
        &self.data
    }

    pub fn get(&self, id: &str) -> Option<&Project> {
        self.data.projects.iter().find(|p| p.id == id)
    }

    fn save(&self) -> Result<(), ProjectError> {
        // Se escribe a un archivo temporal y se renombra: nunca queda la lista a medias.
        let tmp = self.file.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&self.data)?)?;
        fs::rename(&tmp, &self.file)?;
        Ok(())
    }

    fn clean_relay(relay: Vec<String>) -> Result<Vec<String>, ProjectError> {
        let mut out: Vec<String> = Vec::new();
        for r in relay {
            if !KNOWN_PROVIDERS.contains(&r.as_str()) {
                return invalid("Esa IA todavía no está disponible.");
            }
            if !out.contains(&r) {
                out.push(r);
            }
        }
        if out.is_empty() {
            return invalid("Elige al menos una IA para el proyecto.");
        }
        Ok(out)
    }

    fn clean_name(name: &str) -> Result<String, ProjectError> {
        let n = name.trim();
        if n.is_empty() {
            return invalid("Ponle un nombre al proyecto.");
        }
        Ok(n.chars().take(60).collect())
    }

    pub fn create(&mut self, name: &str, path: &Path, relay: Vec<String>) -> Result<Project, ProjectError> {
        let name = Self::clean_name(name)?;
        let relay = Self::clean_relay(relay)?;
        let path = match fs::canonicalize(path) {
            Ok(p) if p.is_dir() => p,
            _ => return invalid("Esa carpeta no existe o no se puede abrir."),
        };
        if path.parent().is_none() {
            return invalid("Elige una carpeta concreta, no la raíz del sistema.");
        }
        if self.data.projects.iter().any(|p| p.path == path) {
            return invalid("Esa carpeta ya es un proyecto.");
        }
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let project = Project { id: format!("p{nanos:x}"), name, path, relay };
        self.data.projects.push(project.clone());
        self.save()?;
        Ok(project)
    }

    pub fn update(&mut self, id: &str, name: Option<&str>, relay: Option<Vec<String>>) -> Result<Project, ProjectError> {
        let name = name.map(Self::clean_name).transpose()?;
        let relay = relay.map(Self::clean_relay).transpose()?;
        let Some(p) = self.data.projects.iter_mut().find(|p| p.id == id) else {
            return invalid("No se encontró el proyecto.");
        };
        if let Some(n) = name {
            p.name = n;
        }
        if let Some(r) = relay {
            p.relay = r;
        }
        let out = p.clone();
        self.save()?;
        Ok(out)
    }

    /// Quita el proyecto de CoModel. No borra ningún archivo de la carpeta.
    pub fn remove(&mut self, id: &str) -> Result<(), ProjectError> {
        let before = self.data.projects.len();
        self.data.projects.retain(|p| p.id != id);
        if self.data.projects.len() == before {
            return invalid("No se encontró el proyecto.");
        }
        self.save()
    }

    /// Orden de relevo para los chats sin carpeta.
    pub fn set_default_relay(&mut self, relay: Vec<String>) -> Result<(), ProjectError> {
        self.data.default_relay = Self::clean_relay(relay)?;
        self.save()
    }

    /// Busca un proyecto por su carpeta (ya canonizada).
    pub fn find_by_path(&self, path: &Path) -> Option<&Project> {
        let canon = fs::canonicalize(path).ok()?;
        self.data.projects.iter().find(|p| p.path == canon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("comodel-proj-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn sub(&self, name: &str) -> PathBuf {
            let p = self.0.join(name);
            fs::create_dir_all(&p).unwrap();
            p
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn relay(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn creates_persists_and_reloads() {
        let t = Tmp::new("a");
        let mut s = ProjectStore::open(&t.sub("cfg")).unwrap();
        let p = s.create("  Mi app  ", &t.sub("app"), relay(&["claude", "openrouter"])).unwrap();
        assert_eq!(p.name, "Mi app");
        assert_eq!(s.state().default_relay, vec!["claude".to_string()]);
        assert_eq!(s.find_by_path(&t.0.join("app")).map(|x| x.id.clone()), Some(p.id.clone()));

        let reopened = ProjectStore::open(&t.0.join("cfg")).unwrap();
        assert_eq!(reopened.state().projects, vec![p.clone()]);
        assert_eq!(reopened.get(&p.id).unwrap().relay, relay(&["claude", "openrouter"]));
    }

    #[test]
    fn rejects_bad_input_with_readable_messages() {
        let t = Tmp::new("b");
        let mut s = ProjectStore::open(&t.sub("cfg")).unwrap();
        let ok = t.sub("app");
        for (name, path, r, expect) in [
            ("", ok.clone(), relay(&["claude"]), "nombre"),
            ("x", t.0.join("no-existe"), relay(&["claude"]), "no existe"),
            ("x", ok.clone(), relay(&[]), "al menos una IA"),
            ("x", ok.clone(), relay(&["gemini"]), "no está disponible"),
            ("x", PathBuf::from("/"), relay(&["claude"]), "raíz"),
        ] {
            let err = s.create(name, &path, r).unwrap_err().to_string();
            assert!(err.contains(expect), "{err} no contiene {expect}");
        }
        s.create("uno", &ok, relay(&["claude"])).unwrap();
        let dup = s.create("dos", &ok, relay(&["claude"])).unwrap_err().to_string();
        assert!(dup.contains("ya es un proyecto"));
    }

    #[test]
    fn updates_relay_order_and_removes_without_touching_files() {
        let t = Tmp::new("c");
        let mut s = ProjectStore::open(&t.sub("cfg")).unwrap();
        let dir = t.sub("app");
        fs::write(dir.join("importante.txt"), "no borrar").unwrap();
        let a = s.create("a", &dir, relay(&["claude", "openrouter"])).unwrap();
        let b = s.create("b", &t.sub("otra"), relay(&["claude"])).unwrap();

        let updated = s.update(&a.id, Some("a2"), Some(relay(&["openrouter", "claude", "openrouter"]))).unwrap();
        assert_eq!(updated.name, "a2");
        assert_eq!(updated.relay, relay(&["openrouter", "claude"]), "sin duplicados y en el orden pedido");

        s.set_default_relay(relay(&["openrouter", "claude"])).unwrap();
        assert_eq!(ProjectStore::open(&t.0.join("cfg")).unwrap().state().default_relay, relay(&["openrouter", "claude"]));
        assert!(s.set_default_relay(relay(&[])).is_err());

        s.remove(&b.id).unwrap();
        s.remove(&a.id).unwrap();
        assert!(s.state().projects.is_empty());
        assert_eq!(fs::read_to_string(dir.join("importante.txt")).unwrap(), "no borrar");
    }

    #[test]
    fn a_corrupt_file_is_set_aside_not_lost() {
        let t = Tmp::new("d");
        let cfg = t.sub("cfg");
        fs::write(cfg.join("projects.json"), "{ esto no es json").unwrap();
        let s = ProjectStore::open(&cfg).unwrap();
        assert!(s.state().projects.is_empty());
        assert!(cfg.join("projects.json.dañado").exists());
    }
}
