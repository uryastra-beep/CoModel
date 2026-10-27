//! Títulos cortos para chats y workspaces a partir del primer mensaje.

/// Lo que se le pide a la IA. El mensaje va recortado: basta con el tema.
pub fn title_prompt(message: &str) -> String {
    let msg: String = message.trim().chars().take(600).collect();
    format!(
        "Escribe un título muy corto (de 2 a 5 palabras, en el mismo idioma del mensaje, sin comillas, sin punto final y sin emojis) para una conversación que empieza con este mensaje. Responde SOLO el título.\n\nMensaje:\n{msg}"
    )
}

/// Deja lo que contestó la IA como un título usable, o `None` si no lo es.
pub fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line = line.trim_start_matches(|c: char| c == '#' || c == '*' || c == '-' || c.is_whitespace());
    let lower = line.to_lowercase();
    let line = ["título:", "titulo:", "title:"].iter().find_map(|p| lower.starts_with(p).then(|| line[p.len()..].trim())).unwrap_or(line);
    let line: String = line.chars().filter(|c| !matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’' | '«' | '»' | '`' | '*')).collect();
    let line = line.trim().trim_end_matches(|c: char| matches!(c, '.' | ':' | ';' | ',' | '!' | '?')).trim().to_string();
    let words = line.split_whitespace().count();
    (!line.is_empty() && line.chars().count() <= 60 && words <= 9).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_becomes_a_short_clean_title() {
        assert_eq!(clean_title("\"Receta de pan casero\"").as_deref(), Some("Receta de pan casero"));
        assert_eq!(clean_title("Título: Error en el login.").as_deref(), Some("Error en el login"));
        assert_eq!(clean_title("\n  **Diseño de la tienda**  \nporque el usuario...").as_deref(), Some("Diseño de la tienda"));
        assert_eq!(clean_title("# Plan de la semana").as_deref(), Some("Plan de la semana"));
    }

    #[test]
    fn a_reply_that_is_not_a_title_is_rejected() {
        assert_eq!(clean_title(""), None);
        assert_eq!(clean_title("   \n "), None);
        assert_eq!(clean_title("Claro, aquí tienes un título para tu conversación sobre cocina casera de verdad"), None);
        assert_eq!(clean_title(&"a".repeat(80)), None);
    }

    #[test]
    fn the_prompt_carries_the_message_trimmed() {
        let p = title_prompt(&"x".repeat(2000));
        assert!(p.len() < 900);
        assert!(p.contains("Responde SOLO el título"));
    }
}
