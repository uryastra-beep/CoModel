//! Memoria compartida: SQLite (registro fiable) + `MEMORY.md` (resumen legible).

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::provider::{ChatMessage, Role};

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("base de datos: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("archivo: {0}")]
    Io(#[from] std::io::Error),
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    role TEXT NOT NULL,
    provider TEXT,
    content TEXT NOT NULL,
    interrupted INTEGER NOT NULL DEFAULT 0,
    chat TEXT NOT NULL DEFAULT '',
    boundary INTEGER NOT NULL DEFAULT 0,
    attachments TEXT NOT NULL DEFAULT '[]',
    outputs TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE IF NOT EXISTS files (
    path TEXT PRIMARY KEY,
    provider TEXT,
    ts INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
CREATE TABLE IF NOT EXISTS usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    chat TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS chats (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sessions (
    chat TEXT NOT NULL,
    provider TEXT NOT NULL,
    session_id TEXT NOT NULL,
    upto INTEGER NOT NULL,
    PRIMARY KEY (chat, provider)
);
CREATE TABLE IF NOT EXISTS handoffs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    from_provider TEXT NOT NULL,
    to_provider TEXT NOT NULL,
    reason TEXT NOT NULL
);
";

const MD_TEMPLATE: &str = "# Memoria del proyecto\n\n## Resumen\n\n## Pendiente\n\n## Cambios\n";

/// La memoria del proyecto es compartida (archivos, relevos, `MEMORY.md`), pero el historial
/// de mensajes es de cada chat.
/// Resumen de uso del proyecto en los últimos días, para la pantalla de Ajustes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageSummary {
    pub days: u32,
    pub totals: UsageTotals,
    /// Un elemento por día, del más antiguo al de hoy, incluidos los días sin uso.
    pub by_day: Vec<DayUsage>,
    pub by_model: Vec<ModelUsage>,
    pub by_chat: Vec<ChatUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageTotals {
    pub turns: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayUsage {
    /// `AAAA-MM-DD`, en hora local.
    pub day: String,
    pub turns: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelUsage {
    pub model: String,
    pub provider: String,
    pub turns: u64,
    pub tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatUsage {
    pub chat: String,
    pub title: String,
    pub turns: u64,
    pub tokens: u64,
    pub cost_usd: f64,
}

impl UsageSummary {
    /// Suma varios resúmenes (uno por proyecto) en uno solo. Los días se alinean por fecha.
    pub fn merge(days: u32, parts: Vec<UsageSummary>) -> UsageSummary {
        let mut totals = UsageTotals { turns: 0, input_tokens: 0, output_tokens: 0, cost_usd: 0.0 };
        let mut by_day: Vec<DayUsage> = Vec::new();
        let mut models: HashMap<(String, String), ModelUsage> = HashMap::new();
        let mut chats: Vec<ChatUsage> = Vec::new();
        for p in parts {
            totals.turns += p.totals.turns;
            totals.input_tokens += p.totals.input_tokens;
            totals.output_tokens += p.totals.output_tokens;
            totals.cost_usd += p.totals.cost_usd;
            if by_day.is_empty() {
                by_day = p.by_day;
            } else {
                for d in p.by_day {
                    match by_day.iter_mut().find(|x| x.day == d.day) {
                        Some(x) => {
                            x.turns += d.turns;
                            x.tokens += d.tokens;
                        }
                        None => by_day.push(d),
                    }
                }
            }
            for m in p.by_model {
                let e = models.entry((m.model.clone(), m.provider.clone())).or_insert(ModelUsage { turns: 0, tokens: 0, cost_usd: 0.0, ..m.clone() });
                e.turns += m.turns;
                e.tokens += m.tokens;
                e.cost_usd += m.cost_usd;
            }
            chats.extend(p.by_chat);
        }
        by_day.sort_by(|a, b| a.day.cmp(&b.day));
        let mut by_model: Vec<ModelUsage> = models.into_values().collect();
        by_model.sort_by(|a, b| b.tokens.cmp(&a.tokens));
        by_model.truncate(10);
        chats.sort_by(|a, b| b.tokens.cmp(&a.tokens));
        chats.truncate(8);
        UsageSummary { days, totals, by_day, by_model, by_chat: chats }
    }
}

pub struct Memory {
    conn: Connection,
    md_path: PathBuf,
    chat: String,
}

impl Memory {
    /// Abre (o crea) la memoria en `<proyecto>/.comodel/` para el chat `chat`.
    pub fn open(project_dir: &Path, chat: &str) -> Result<Self, MemoryError> {
        let dir = project_dir.join(".comodel");
        fs::create_dir_all(&dir)?;
        let conn = Connection::open(dir.join("memory.db"))?;
        // Varios chats escriben a la vez: esperar el candado en vez de fallar.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        // Bases creadas antes de existir `chat`: añadir la columna. En una base nueva ya existe y falla sin daño.
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN chat TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN boundary INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN attachments TEXT NOT NULL DEFAULT '[]'", []);
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN outputs TEXT NOT NULL DEFAULT '[]'", []);
        // La base de datos es de esta máquina: se evita que acabe en el repositorio del usuario.
        let ignore = dir.join(".gitignore");
        if !ignore.exists() {
            fs::write(&ignore, "memory.db*\n")?;
        }
        let md_path = dir.join("MEMORY.md");
        if !md_path.exists() {
            fs::write(&md_path, MD_TEMPLATE)?;
        }
        Ok(Self { conn, md_path, chat: chat.to_string() })
    }

    /// Copia el historial completo con sus cortes de contexto. La memoria original se conserva.
    pub fn copy_chat_to(&self, project_dir: &Path) -> Result<(), MemoryError> {
        let dest = Memory::open(project_dir, &self.chat)?;
        let source = self.md_path.with_file_name("memory.db");
        dest.conn.execute("ATTACH DATABASE ?1 AS incoming", params![source.to_string_lossy()])?;
        let tx = dest.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM messages WHERE chat = ?1", params![self.chat])?;
        tx.execute("INSERT INTO messages (ts,role,provider,content,interrupted,chat,boundary,attachments,outputs) SELECT ts,role,provider,content,interrupted,chat,boundary,attachments,outputs FROM incoming.messages WHERE chat = ?1 ORDER BY id", params![self.chat])?;
        tx.execute("INSERT OR REPLACE INTO chats (id,title) SELECT id,title FROM incoming.chats WHERE id = ?1", params![self.chat])?;
        // Los adjuntos relativos siguen apuntando al archivo original al cambiar de carpeta.
        let source_root = self.md_path.parent().and_then(Path::parent).unwrap_or(Path::new(""));
        let messages: Vec<(i64, String)> = {
            let mut stmt = tx.prepare("SELECT id, content FROM messages WHERE chat = ?1")?;
            let rows = stmt.query_map(params![self.chat], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
            rows
        };
        for (id, content) in messages {
            if let Some((text, list)) = content.split_once("\n\n[Archivos adjuntos]\n") {
                let paths = list.lines().map(|line| match line.strip_prefix("- ") {
                    Some(path) if Path::new(path).is_relative() => format!("- {}", source_root.join(path).display()),
                    _ => line.to_string(),
                }).collect::<Vec<_>>().join("\n");
                tx.execute("UPDATE messages SET content = ?1 WHERE id = ?2", params![format!("{text}\n\n[Archivos adjuntos]\n{paths}"), id])?;
            }
        }
        // Los archivos que la IA entregó viajan con el chat: se copian a la carpeta nueva con la misma ruta relativa.
        let outputs: Vec<String> = {
            let mut stmt = tx.prepare("SELECT outputs FROM messages WHERE chat = ?1 AND outputs != '[]'")?;
            let rows = stmt.query_map(params![self.chat], |r| r.get(0))?.collect::<Result<_, _>>()?;
            rows
        };
        for json in outputs {
            for f in serde_json::from_str::<Vec<crate::outputs::OutFile>>(&json).unwrap_or_default() {
                if let Ok((from, rel)) = crate::outputs::resolve_in_root(source_root, &f.path) {
                    let to = project_dir.join(&rel);
                    if !to.exists() {
                        if let Some(parent) = to.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let _ = fs::copy(from, to);
                    }
                }
            }
        }
        // El uso pasado permanece en su base original para no contarlo dos veces en el resumen global.
        tx.commit()?;
        Ok(())
    }

    /// Id del último mensaje del chat (0 si no hay).
    pub fn last_message_id(&self) -> Result<i64, MemoryError> {
        Ok(self.conn.query_row("SELECT COALESCE(MAX(id), 0) FROM messages WHERE chat = ?1", params![self.chat], |r| r.get(0))?)
    }

    /// Recuerda la sesión del CLI de `provider` y hasta qué mensaje la conoce.
    pub fn save_session(&self, provider: &str, session_id: &str, upto: i64) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO sessions (chat, provider, session_id, upto) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(chat, provider) DO UPDATE SET session_id = ?3, upto = ?4",
            params![self.chat, provider, session_id, upto],
        )?;
        Ok(())
    }

    pub fn forget_session(&self, provider: &str) -> Result<(), MemoryError> {
        self.conn.execute("DELETE FROM sessions WHERE chat = ?1 AND provider = ?2", params![self.chat, provider])?;
        Ok(())
    }

    /// La sesión de `provider` y los mensajes del chat posteriores a su último turno y anteriores a `before`.
    /// Un resumen o un borrado de contexto posterior la invalidan: devuelve `None`.
    pub fn session_for(&self, provider: &str, before: i64) -> Result<Option<(String, Vec<ChatMessage>)>, MemoryError> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row("SELECT session_id, upto FROM sessions WHERE chat = ?1 AND provider = ?2", params![self.chat, provider], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((sid, upto)) = row else { return Ok(None) };
        let cut: i64 = self.conn.query_row("SELECT COALESCE(MAX(id), 0) FROM messages WHERE chat = ?1 AND boundary > 0", params![self.chat], |r| r.get(0))?;
        if cut >= upto {
            self.forget_session(provider)?;
            return Ok(None);
        }
        let mut stmt = self.conn.prepare("SELECT role, provider, content, attachments FROM messages WHERE chat = ?1 AND id > ?2 AND id < ?3 AND interrupted = 0 AND boundary = 0 ORDER BY id")?;
        let missed = stmt
            .query_map(params![self.chat, upto, before], |r| {
                let role: String = r.get(0)?;
                Ok(ChatMessage {
                    role: if role == "user" { Role::User } else { Role::Assistant },
                    provider: r.get(1)?,
                    content: r.get(2)?,
                    attachments: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                    outputs: Vec::new(),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(Some((sid, missed)))
    }

    /// El último mensaje del chat pasa a ser el resumen que sustituye a lo anterior.
    pub fn mark_summary(&self) -> Result<(), MemoryError> {
        self.conn.execute(
            "UPDATE messages SET boundary = 1 WHERE id = (SELECT MAX(id) FROM messages WHERE chat = ?1)",
            params![self.chat],
        )?;
        Ok(())
    }

    /// Lo anterior deja de formar parte del contexto; el historial guardado se conserva.
    pub fn clear_context(&self) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO messages (role, content, chat, boundary) VALUES ('user', '', ?1, 2)",
            params![self.chat],
        )?;
        Ok(())
    }

    pub fn md_path(&self) -> &Path {
        &self.md_path
    }

    pub fn add_message(
        &self,
        role: Role,
        provider: Option<&str>,
        content: &str,
        interrupted: bool,
    ) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO messages (role, provider, content, interrupted, chat) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![role.as_str(), provider, content, interrupted, self.chat],
        )?;
        Ok(())
    }

    /// Las rutas vienen de la validación de adjuntos de CoModel, nunca del texto de un mensaje.
    pub fn attach_latest(&self, paths: &[PathBuf]) -> Result<(), MemoryError> {
        let data = serde_json::to_string(paths).unwrap_or_else(|_| "[]".into());
        self.conn.execute("UPDATE messages SET attachments = ?1 WHERE id = (SELECT MAX(id) FROM messages WHERE chat = ?2 AND role = 'user')", params![data, self.chat])?;
        Ok(())
    }

    /// Deja los archivos que entregó la IA asociados a su último mensaje (el que se acaba de guardar).
    pub fn attach_outputs_latest(&self, files: &[crate::outputs::OutFile]) -> Result<(), MemoryError> {
        if files.is_empty() {
            return Ok(());
        }
        let data = serde_json::to_string(files).unwrap_or_else(|_| "[]".into());
        self.conn.execute("UPDATE messages SET outputs = ?1 WHERE id = (SELECT MAX(id) FROM messages WHERE chat = ?2 AND role = 'assistant')", params![data, self.chat])?;
        Ok(())
    }

    /// Carpeta de trabajo a la que pertenece esta memoria.
    pub fn root(&self) -> &Path {
        self.md_path.parent().and_then(Path::parent).unwrap_or(Path::new(""))
    }

    /// Los últimos `n` mensajes completos (los interrumpidos no cuentan), del más viejo al más nuevo.
    /// Solo cuenta lo posterior al último corte de contexto: un resumen (`/compact`, se incluye)
    /// o un borrado (`/clear`, no se incluye).
    pub fn recent_messages(&self, n: usize) -> Result<Vec<ChatMessage>, MemoryError> {
        let mut stmt = self.conn.prepare(
            "SELECT role, provider, content, attachments, outputs FROM messages
             WHERE interrupted = 0 AND chat = ?1 AND boundary < 2
               AND id >= COALESCE((SELECT MAX(id) FROM messages WHERE chat = ?1 AND boundary > 0), 0)
             ORDER BY id DESC LIMIT ?2",
        )?;
        let mut msgs: Vec<ChatMessage> = stmt
            .query_map(params![self.chat, n as i64], |r| {
                let role: String = r.get(0)?;
                Ok(ChatMessage {
                    role: if role == "user" { Role::User } else { Role::Assistant },
                    provider: r.get(1)?,
                    content: r.get(2)?,
                    attachments: serde_json::from_str(&r.get::<_,String>(3)?).unwrap_or_default(),
                    outputs: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                })
            })?
            .collect::<Result<_, _>>()?;
        msgs.reverse();
        Ok(msgs)
    }

    /// Contexto acotado; el historial del chat receptor se mantiene independiente.
    pub fn shared_snapshot(&self) -> Result<String, MemoryError> {
        let mut out: String = self.read_md()?.chars().take(6000).collect();
        for m in self.recent_messages(12)? {
            out.push_str(&format!("\n{}: {}\n", m.role.as_str(), m.content.chars().take(2000).collect::<String>()));
        }
        Ok(out)
    }

    pub fn touch_file(&self, path: &str, provider: &str) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO files (path, provider) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET provider = ?2, ts = strftime('%s','now')",
            params![path, provider],
        )?;
        Ok(())
    }

    pub fn files_touched(&self) -> Result<Vec<String>, MemoryError> {
        let mut stmt = self.conn.prepare("SELECT path FROM files ORDER BY ts, path")?;
        let rows = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Guarda el uso de un turno. `cost_usd` es el coste que informa el proveedor (0 si no lo informa).
    pub fn record_usage(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    ) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO usage (chat, provider, model, input_tokens, output_tokens, cost_usd) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![self.chat, provider, model, input_tokens as i64, output_tokens as i64, cost_usd],
        )?;
        Ok(())
    }

    /// Nombre con el que se muestra este chat en el resumen de uso.
    pub fn set_chat_title(&self, title: &str) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO chats (id, title) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET title = ?2",
            params![self.chat, title],
        )?;
        Ok(())
    }

    /// Uso de los últimos `days` días (incluido hoy): totales, por día, por modelo y por chat.
    pub fn usage_summary(&self, days: u32) -> Result<UsageSummary, MemoryError> {
        let days = days.clamp(1, 90);
        let since = format!("-{} days", days - 1);

        let totals = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), COALESCE(SUM(cost_usd),0)
             FROM usage WHERE date(ts,'unixepoch','localtime') >= date('now','localtime', ?1)",
            params![since],
            |r| {
                Ok(UsageTotals {
                    turns: r.get::<_, i64>(0)? as u64,
                    input_tokens: r.get::<_, i64>(1)? as u64,
                    output_tokens: r.get::<_, i64>(2)? as u64,
                    cost_usd: r.get(3)?,
                })
            },
        )?;

        let mut per_day: HashMap<String, (u64, u64)> = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT date(ts,'unixepoch','localtime') AS d, COUNT(*), SUM(input_tokens + output_tokens)
             FROM usage WHERE d >= date('now','localtime', ?1) GROUP BY d",
        )?;
        for row in stmt.query_map(params![since], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)? as u64))
        })? {
            let (d, turns, tokens) = row?;
            per_day.insert(d, (turns, tokens));
        }
        let mut by_day = Vec::new();
        for i in (0..days).rev() {
            let day: String = self.conn.query_row("SELECT date('now','localtime', ?1)", params![format!("-{i} days")], |r| r.get(0))?;
            let (turns, tokens) = per_day.get(&day).copied().unwrap_or((0, 0));
            by_day.push(DayUsage { day, turns, tokens });
        }

        let mut stmt = self.conn.prepare(
            "SELECT model, provider, COUNT(*), SUM(input_tokens + output_tokens) AS t, SUM(cost_usd)
             FROM usage WHERE date(ts,'unixepoch','localtime') >= date('now','localtime', ?1)
             GROUP BY model, provider ORDER BY t DESC LIMIT 10",
        )?;
        let by_model = stmt
            .query_map(params![since], |r| {
                Ok(ModelUsage {
                    model: r.get(0)?,
                    provider: r.get(1)?,
                    turns: r.get::<_, i64>(2)? as u64,
                    tokens: r.get::<_, i64>(3)? as u64,
                    cost_usd: r.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT u.chat, COALESCE(c.title, u.chat), COUNT(*), SUM(u.input_tokens + u.output_tokens) AS t, SUM(u.cost_usd)
             FROM usage u LEFT JOIN chats c ON c.id = u.chat
             WHERE date(u.ts,'unixepoch','localtime') >= date('now','localtime', ?1)
             GROUP BY u.chat ORDER BY t DESC LIMIT 8",
        )?;
        let by_chat = stmt
            .query_map(params![since], |r| {
                Ok(ChatUsage {
                    chat: r.get(0)?,
                    title: r.get(1)?,
                    turns: r.get::<_, i64>(2)? as u64,
                    tokens: r.get::<_, i64>(3)? as u64,
                    cost_usd: r.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        Ok(UsageSummary { days, totals, by_day, by_model, by_chat })
    }

    pub fn record_handoff(&self, from: &str, to: &str, reason: &str) -> Result<(), MemoryError> {
        self.conn.execute(
            "INSERT INTO handoffs (from_provider, to_provider, reason) VALUES (?1, ?2, ?3)",
            params![from, to, reason],
        )?;
        Ok(())
    }

    pub fn handoff_count(&self) -> Result<u64, MemoryError> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM handoffs", [], |r| r.get::<_, i64>(0))? as u64)
    }

    pub fn read_md(&self) -> Result<String, MemoryError> {
        Ok(fs::read_to_string(&self.md_path)?)
    }

    /// Líneas con contenido bajo `## <heading>` en `MEMORY.md`, sin viñetas.
    pub fn md_section(&self, heading: &str) -> Result<Vec<String>, MemoryError> {
        let md = self.read_md()?;
        let want = format!("## {heading}");
        let mut inside = false;
        let mut out = Vec::new();
        for line in md.lines() {
            if line.starts_with("## ") {
                inside = line.trim() == want;
                continue;
            }
            if inside {
                let t = line.trim().trim_start_matches(['-', '*']).trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
            }
        }
        Ok(out)
    }

    /// Añade una nota de CoModel al final de `MEMORY.md` (registro de relevos).
    pub fn append_md_note(&self, note: &str) -> Result<(), MemoryError> {
        let mut md = self.read_md()?;
        if !md.ends_with('\n') {
            md.push('\n');
        }
        md.push_str(&format!("- {note}\n"));
        fs::write(&self.md_path, md)?;
        Ok(())
    }
}
