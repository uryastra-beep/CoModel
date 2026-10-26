//! Respuestas del usuario a lo que pide una IA a mitad de un turno (permisos y preguntas).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use tokio::sync::oneshot;

/// Lo que responde el usuario.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Answer {
    /// Permitir. Con `always`, se permiten también las próximas ediciones de la sesión.
    Allow {
        #[serde(default)]
        always: bool,
    },
    Deny {
        #[serde(default)]
        message: Option<String>,
    },
    /// Respuestas a las preguntas: texto de la pregunta -> opción elegida (varias separadas por coma).
    Answers { answers: HashMap<String, String> },
}

/// Peticiones pendientes de respuesta. Se comparte entre la interfaz y los adaptadores.
#[derive(Clone, Default)]
pub struct Interactions {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Answer>>>>,
}

impl std::fmt::Debug for Interactions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interactions").finish_non_exhaustive()
    }
}

impl Interactions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apunta una petición y devuelve por dónde llegará la respuesta. Si quien espera desaparece
    /// (el turno se paró), la entrada se limpia sola en la siguiente petición.
    pub fn register(&self, id: &str) -> oneshot::Receiver<Answer> {
        let (tx, rx) = oneshot::channel();
        let mut map = self.pending.lock().expect("interactions");
        map.retain(|_, s| !s.is_closed());
        map.insert(id.to_string(), tx);
        rx
    }

    /// Entrega la respuesta. `false` si esa petición ya no existe (ya respondida o cancelada).
    pub fn answer(&self, id: &str, answer: Answer) -> bool {
        let sender = self.pending.lock().expect("interactions").remove(id);
        sender.is_some_and(|s| s.send(answer).is_ok())
    }

    pub fn pending_count(&self) -> usize {
        let mut map = self.pending.lock().expect("interactions");
        map.retain(|_, s| !s.is_closed());
        map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn answers_reach_the_waiter_once() {
        let i = Interactions::new();
        let rx = i.register("a");
        assert!(i.answer("a", Answer::Allow { always: false }));
        assert_eq!(rx.await.unwrap(), Answer::Allow { always: false });
        assert!(!i.answer("a", Answer::Allow { always: false }), "no se responde dos veces");
        assert!(!i.answer("nope", Answer::Deny { message: None }));
    }

    #[tokio::test]
    async fn abandoned_requests_are_cleaned_up() {
        let i = Interactions::new();
        let rx = i.register("a");
        drop(rx); // el turno se paró
        assert_eq!(i.pending_count(), 0);
        assert!(!i.answer("a", Answer::Allow { always: false }));
    }

    #[test]
    fn answers_deserialize_from_the_ui_shape() {
        let a: Answer = serde_json::from_str(r#"{"kind":"allow","always":true}"#).unwrap();
        assert_eq!(a, Answer::Allow { always: true });
        let a: Answer = serde_json::from_str(r#"{"kind":"deny"}"#).unwrap();
        assert_eq!(a, Answer::Deny { message: None });
        let a: Answer = serde_json::from_str(r#"{"kind":"answers","answers":{"¿Rojo o azul?":"Rojo"}}"#).unwrap();
        assert!(matches!(a, Answer::Answers { answers } if answers["¿Rojo o azul?"] == "Rojo"));
    }
}
