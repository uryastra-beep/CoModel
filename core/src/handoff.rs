//! Paquete de relevo: lo que recibe el siguiente proveedor para seguir donde quedó el anterior.
//! El historial viaja aparte, en `TurnRequest::history`.

use crate::memory::{Memory, MemoryError};

#[derive(Debug, Clone, PartialEq)]
pub struct HandoffPackage {
    pub from: String,
    pub to: String,
    pub reason: String,
    pub summary: String,
    pub files: Vec<String>,
    pub pending: Vec<String>,
    /// Lo que alcanzó a escribir el proveedor agotado antes de cortarse.
    pub partial: Option<String>,
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

impl HandoffPackage {
    /// Arma el paquete con la memoria del proyecto. Los pendientes salen de `MEMORY.md`
    /// (si la IA los mantuvo) más la petición que quedó interrumpida, que CoModel sí conoce.
    pub fn build(
        memory: &Memory,
        from: &str,
        to: &str,
        reason: &str,
        interrupted_prompt: &str,
        partial: Option<String>,
    ) -> Result<Self, MemoryError> {
        let mut summary = memory.md_section("Resumen")?.join(" ");
        if summary.is_empty() {
            // La IA no escribió resumen: CoModel genera uno mínimo a partir del historial.
            let recent = memory.recent_messages(4)?;
            summary = recent
                .iter()
                .map(|m| format!("[{}] {}", m.role.as_str(), clip(&m.content, 200)))
                .collect::<Vec<_>>()
                .join(" | ");
        }
        let mut pending = memory.md_section("Pendiente")?;
        pending.push(format!("Petición interrumpida por el límite de {from}: {}", clip(interrupted_prompt, 300)));
        Ok(Self {
            from: from.into(),
            to: to.into(),
            reason: reason.into(),
            summary,
            files: memory.files_touched()?,
            pending,
            partial: partial.filter(|p| !p.trim().is_empty()),
        })
    }

    pub fn render(&self) -> String {
        let mut s = format!(
            "# Relevo de proveedor\nContinúas el trabajo de `{}` (se quedó sin uso: {}). Sigue donde quedó, sin repetir lo ya hecho.\n\n## Resumen\n{}\n",
            self.from, self.reason, if self.summary.is_empty() { "(sin resumen)" } else { &self.summary }
        );
        s.push_str("\n## Archivos tocados\n");
        if self.files.is_empty() {
            s.push_str("(ninguno)\n");
        }
        for f in &self.files {
            s.push_str(&format!("- {f}\n"));
        }
        s.push_str("\n## Pendiente\n");
        for p in &self.pending {
            s.push_str(&format!("- {p}\n"));
        }
        if let Some(p) = &self.partial {
            s.push_str(&format!("\n## Respuesta parcial de {}\n{}\n", self.from, clip(p, 2000)));
        }
        s
    }
}
