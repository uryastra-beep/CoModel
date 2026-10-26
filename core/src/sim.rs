//! Proveedor de desarrollo que simula un límite de uso para probar el relevo.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::events::ProviderEvent;
use crate::provider::{Provider, ProviderError, TurnRequest};

/// Envuelve un proveedor real y, en la llamada número `trip_on_call` (empezando en 1),
/// responde con `LimitReached` sin llamar al proveedor de dentro.
pub struct LimitOnCall {
    inner: Arc<dyn Provider>,
    trip_on_call: usize,
    calls: AtomicUsize,
}

impl LimitOnCall {
    pub fn new(inner: Arc<dyn Provider>, trip_on_call: usize) -> Self {
        Self { inner, trip_on_call, calls: AtomicUsize::new(0) }
    }
}

#[async_trait]
impl Provider for LimitOnCall {
    fn id(&self) -> &str {
        self.inner.id()
    }

    async fn run_turn(
        &self,
        req: TurnRequest,
        tx: mpsc::Sender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.trip_on_call {
            let _ = tx
                .send(ProviderEvent::LimitReached { reason: "límite simulado".into(), resets_at: None })
                .await;
            return Ok(());
        }
        self.inner.run_turn(req, tx).await
    }
}
