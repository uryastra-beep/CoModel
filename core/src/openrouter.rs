//! Adaptador de OpenRouter por API (compatible con OpenAI, respuesta en streaming SSE).

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::events::ProviderEvent;
use crate::provider::{ProviderError, Provider, TurnRequest};

pub struct OpenRouter {
    pub id: String,
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
}

impl OpenRouter {
    pub fn new(id: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            default_model: "openai/gpt-4o-mini".into(),
        }
    }

    /// El modelo del chat puede ser de otro proveedor (tras un relevo): los ids de OpenRouter
    /// llevan `/` (`vendedor/modelo`); cualquier otro se cambia por el modelo de respaldo.
    fn model_for(&self, req: &TurnRequest) -> String {
        match &req.model {
            Some(m) if m.contains('/') => m.clone(),
            _ => self.default_model.clone(),
        }
    }

    fn body(&self, req: &TurnRequest) -> Value {
        let mut messages = Vec::new();
        if let Some(p) = &req.preamble {
            messages.push(json!({"role": "system", "content": p}));
        }
        for m in &req.history {
            messages.push(json!({"role": m.role.as_str(), "content": m.content}));
        }
        messages.push(json!({"role": "user", "content": req.prompt}));
        let mut body = json!({
            "model": self.model_for(req),
            "messages": messages,
            "stream": true,
            "usage": {"include": true},
        });
        if let Some(e) = &req.effort {
            body["reasoning"] = json!({"effort": e});
        }
        body
    }

    /// Créditos restantes de la clave (`GET /key`). Es opcional: si falla, no pasa nada.
    async fn quota(&self, client: &reqwest::Client) -> Option<ProviderEvent> {
        let v: Value = client
            .get(format!("{}/key", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let data = &v["data"];
        let limit = data["limit"].as_f64()?;
        let usage = data["usage"].as_f64()?;
        (limit > 0.0).then(|| ProviderEvent::Quota {
            window: "credits".into(),
            used_fraction: (usage / limit).clamp(0.0, 1.0),
            resets_at: None,
        })
    }
}

/// Datos de la clave (`GET /key`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct KeyInfo {
    pub label: Option<String>,
    /// Dólares gastados con esta clave.
    pub usage: f64,
    /// Límite de gasto de la clave, si tiene uno.
    pub limit: Option<f64>,
}

impl OpenRouter {
    /// Comprueba que la clave funciona. El error es un texto para mostrar al usuario y nunca incluye la clave.
    pub async fn key_info(&self) -> Result<KeyInfo, String> {
        let resp = reqwest::Client::new()
            .get(format!("{}/key", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|_| "No se pudo conectar con OpenRouter. Revisa tu conexión a internet.".to_string())?;
        match resp.status().as_u16() {
            200 => {}
            401 | 403 => return Err("OpenRouter rechazó la clave. Comprueba que está completa y que sigue activa.".into()),
            code => return Err(format!("OpenRouter respondió con el error {code}.")),
        }
        let v: Value = resp.json().await.map_err(|_| "Respuesta inesperada de OpenRouter.".to_string())?;
        let d = &v["data"];
        Ok(KeyInfo { label: d["label"].as_str().map(String::from), usage: d["usage"].as_f64().unwrap_or(0.0), limit: d["limit"].as_f64() })
    }
}

/// Traduce un error de OpenRouter (HTTP o a mitad del stream) a un evento.
pub fn classify_error(code: u64, message: &str) -> ProviderEvent {
    match code {
        // 429: límite de peticiones. 402: créditos agotados.
        429 | 402 => ProviderEvent::LimitReached { reason: format!("{code}: {message}"), resets_at: None },
        _ => ProviderEvent::Error(format!("{code}: {message}")),
    }
}

/// Traduce una línea SSE (`data: {...}`) a eventos. `None` en `[DONE]`.
/// Los comentarios (`: OPENROUTER PROCESSING`) y las líneas vacías no producen eventos.
pub fn parse_sse_line(line: &str) -> Option<Vec<ProviderEvent>> {
    let Some(data) = line.strip_prefix("data:") else {
        return Some(Vec::new());
    };
    let data = data.trim();
    if data == "[DONE]" {
        return None;
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else {
        return Some(Vec::new());
    };
    let mut out = Vec::new();
    if let Some(err) = v.get("error") {
        out.push(classify_error(
            err["code"].as_u64().unwrap_or(0),
            err["message"].as_str().unwrap_or("error desconocido"),
        ));
        return Some(out);
    }
    if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
        if !t.is_empty() {
            out.push(ProviderEvent::Text(t.to_string()));
        }
    }
    if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
        out.push(ProviderEvent::Usage {
            input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
            output_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
            // El stream no trae el tamaño de contexto; habría que pedirlo a /models.
            context_window: None,
            cost_usd: u["cost"].as_f64(),
        });
    }
    Some(out)
}

#[async_trait]
impl Provider for OpenRouter {
    fn id(&self) -> &str {
        &self.id
    }

    fn reads_files(&self) -> bool {
        false
    }

    async fn run_turn(
        &self,
        req: TurnRequest,
        tx: mpsc::Sender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .header("X-Title", "CoModel")
            .json(&self.body(&req))
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or(text);
            let _ = tx.send(classify_error(status.as_u16().into(), &message)).await;
            return Ok(());
        }

        let _ = tx
            .send(ProviderEvent::Started { session_id: None, model: Some(self.model_for(&req)), mode: None })
            .await;

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut failed = false;
        'outer: while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Http(e.to_string()))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buf.find('\n') {
                let line: String = buf.drain(..=pos).collect();
                let Some(events) = parse_sse_line(line.trim()) else {
                    break 'outer;
                };
                for ev in events {
                    failed |= matches!(ev, ProviderEvent::LimitReached { .. } | ProviderEvent::Error(_));
                    if tx.send(ev).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
        if !failed {
            if let Some(q) = self.quota(&client).await {
                let _ = tx.send(q).await;
            }
            let _ = tx.send(ProviderEvent::Finished).await;
        }
        Ok(())
    }
}
