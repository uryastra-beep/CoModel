//! Terminales reales dentro de CoModel: un shell por panel, en un pseudoterminal.
//! La salida llega a la interfaz en base64 por el evento `comodel://term`, para no partir
//! caracteres UTF-8 a la mitad.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;

pub const TERM_EVENT: &str = "comodel://term";

#[derive(Serialize, Clone, Debug)]
pub struct TermEvent {
    pub id: String,
    /// Salida del shell, en base64.
    pub data: Option<String>,
    /// El shell terminó.
    pub exit: bool,
}

struct Term {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct Terminals {
    map: Mutex<HashMap<String, Term>>,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.clamp(2, 500), cols: cols.clamp(2, 1000), pixel_width: 0, pixel_height: 0 }
}

/// El shell del usuario, o `sh` si no se sabe cuál es.
fn shell() -> String {
    std::env::var("SHELL").ok().filter(|s| std::path::Path::new(s).is_file()).unwrap_or_else(|| "/bin/sh".into())
}

impl Terminals {
    /// Abre un shell en `cwd` (o en la carpeta personal). Si ya hay uno con ese id, no hace nada.
    /// `emit` recibe la salida del shell y el aviso de que terminó.
    pub fn open(&self, emit: std::sync::Arc<dyn Fn(TermEvent) + Send + Sync>, id: &str, cwd: Option<PathBuf>, cols: u16, rows: u16) -> Result<(), String> {
        let mut map = self.map.lock().map_err(|_| "terminal ocupada".to_string())?;
        if map.contains_key(id) {
            return Ok(());
        }
        let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| format!("No se pudo abrir la terminal: {e}"))?;
        let mut cmd = CommandBuilder::new(shell());
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let dir = cwd.filter(|d| d.is_dir()).or_else(|| std::env::var_os("HOME").map(PathBuf::from));
        if let Some(d) = dir {
            cmd.cwd(d);
        }
        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("No se pudo iniciar el shell: {e}"))?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let tid = id.to_string();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let data = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                        emit(TermEvent { id: tid.clone(), data: Some(data), exit: false });
                    }
                }
            }
            emit(TermEvent { id: tid, data: None, exit: true });
        });
        map.insert(id.to_string(), Term { master: pair.master, writer, child });
        Ok(())
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), String> {
        let mut map = self.map.lock().map_err(|_| "terminal ocupada".to_string())?;
        let t = map.get_mut(id).ok_or("La terminal ya no existe.")?;
        t.writer.write_all(data).and_then(|_| t.writer.flush()).map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let map = self.map.lock().map_err(|_| "terminal ocupada".to_string())?;
        let t = map.get(id).ok_or("La terminal ya no existe.")?;
        t.master.resize(size(cols, rows)).map_err(|e| e.to_string())
    }

    /// Cierra el shell y todo lo que dejó corriendo en su terminal.
    pub fn close(&self, id: &str) {
        if let Ok(mut map) = self.map.lock() {
            if let Some(mut t) = map.remove(id) {
                let _ = t.child.kill();
            }
        }
    }
}

impl Drop for Terminals {
    fn drop(&mut self) {
        if let Ok(mut map) = self.map.lock() {
            for (_, mut t) in map.drain() {
                let _ = t.child.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;

    fn collect(events: &Arc<Mutex<Vec<TermEvent>>>) -> (String, bool) {
        let mut out = Vec::new();
        let mut exit = false;
        for e in events.lock().unwrap().iter() {
            if let Some(d) = &e.data {
                out.extend(base64::engine::general_purpose::STANDARD.decode(d).unwrap());
            }
            exit |= e.exit;
        }
        (String::from_utf8_lossy(&out).into_owned(), exit)
    }

    fn wait_for(events: &Arc<Mutex<Vec<TermEvent>>>, needle: &str) -> bool {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if collect(events).0.contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn a_real_shell_runs_commands_in_its_folder_and_ends_when_closed() {
        let dir = std::env::temp_dir().join(format!("comodel-term-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let terms = Terminals::default();
        terms.open(Arc::new(move |e| sink.lock().unwrap().push(e)), "t1", Some(dir.clone()), 80, 24).unwrap();

        // La orden se ejecuta en la carpeta pedida y su salida llega codificada.
        terms.write("t1", b"echo hola-$((20+22)); pwd\n").unwrap();
        assert!(wait_for(&events, "hola-42"), "salida: {:?}", collect(&events).0);
        assert!(wait_for(&events, dir.file_name().unwrap().to_str().unwrap()), "no entró en su carpeta: {:?}", collect(&events).0);

        // Cambiar el tamaño se refleja en el shell.
        terms.resize("t1", 100, 30).unwrap();
        terms.write("t1", b"stty size\n").unwrap();
        assert!(wait_for(&events, "30 100"), "tamaño: {:?}", collect(&events).0);

        // Abrir otra vez el mismo id no crea un segundo shell.
        terms.open(Arc::new(|_| {}), "t1", None, 80, 24).unwrap();

        // Cerrarla termina el shell y avisa.
        terms.close("t1");
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end && !collect(&events).1 {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(collect(&events).1, "no avisó de que terminó");
        assert!(terms.write("t1", b"x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
