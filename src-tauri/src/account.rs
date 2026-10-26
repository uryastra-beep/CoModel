//! Cuenta de usuario: correo, Google y GitHub sobre Supabase Auth (API REST).
//!
//! La URL del proyecto y la clave `anon` son públicas por diseño (la seguridad la dan las reglas
//! del servidor), así que pueden ir en el repositorio. La sesión del usuario se guarda en el llavero.
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Se rellenan al crear el proyecto de Supabase (ver docs/ACCOUNTS.md) o con variables de entorno.
const SUPABASE_URL: &str = "https://qazmfxfyjdncmkojxasw.supabase.co";
const SUPABASE_ANON_KEY: &str = "sb_publishable_7FXoG2Y-CEXczKBMnShF6A_jA5DCIz8";

/// Puerto fijo para recibir la vuelta de Google/GitHub; debe estar en las URL de redirección permitidas.
const CALLBACK_PORT: u16 = 53682;

fn config() -> Option<(String, String)> {
    let pick = |env: &str, fallback: &str| std::env::var(env).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| fallback.to_string());
    let url = pick("COMODEL_SUPABASE_URL", SUPABASE_URL);
    let key = pick("COMODEL_SUPABASE_ANON_KEY", SUPABASE_ANON_KEY);
    (!url.is_empty() && !key.is_empty()).then(|| (url.trim_end_matches('/').to_string(), key))
}

pub fn configured() -> bool {
    config().is_some()
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub email: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    /// "email", "google" o "github".
    pub provider: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Session {
    access_token: String,
    refresh_token: String,
}

/// Resultado de crear una cuenta con correo.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignUp {
    SignedIn { account: Account },
    /// El servidor mandó un correo de confirmación.
    ConfirmEmail,
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("comodel", "account").map_err(super::keyring_error)
}

async fn load_session() -> Option<Session> {
    tauri::async_runtime::spawn_blocking(|| entry().ok()?.get_password().ok().and_then(|s| serde_json::from_str(&s).ok()))
        .await
        .ok()
        .flatten()
}

async fn save_session(s: &Session) -> Result<(), String> {
    let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || entry()?.set_password(&json).map_err(super::keyring_error))
        .await
        .map_err(|e| e.to_string())?
}

fn http() -> reqwest::Client {
    reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap_or_default()
}

/// Mensaje en español para el error de Supabase (que llega en inglés).
fn friendly(status: u16, body: &serde_json::Value) -> String {
    let raw = ["msg", "error_description", "message", "error"].iter().find_map(|k| body.get(*k).and_then(|v| v.as_str())).unwrap_or("");
    let l = raw.to_lowercase();
    if l.contains("invalid login") || l.contains("invalid credentials") {
        "Correo o contraseña incorrectos.".into()
    } else if l.contains("already registered") || l.contains("already been registered") {
        "Ese correo ya tiene una cuenta. Inicia sesión.".into()
    } else if l.contains("email not confirmed") {
        "Confirma tu correo con el enlace que te enviamos y vuelve a entrar.".into()
    } else if l.contains("password") && (l.contains("least") || l.contains("weak") || l.contains("short")) {
        "La contraseña es demasiado corta o débil. Usa al menos 8 caracteres.".into()
    } else if l.contains("rate limit") || status == 429 {
        "Demasiados intentos. Espera un momento e inténtalo de nuevo.".into()
    } else if l.contains("valid email") || l.contains("invalid email") {
        "Ese correo no parece válido.".into()
    } else {
        "No se pudo completar. Inténtalo de nuevo.".into()
    }
}

async fn post(path: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let (url, key) = config().ok_or("El inicio de sesión no está configurado en esta compilación.")?;
    let res = http()
        .post(format!("{url}/auth/v1/{path}"))
        .header("apikey", key)
        .json(&body)
        .send()
        .await
        .map_err(|_| "No hay conexión con el servidor de cuentas.".to_string())?;
    let status = res.status().as_u16();
    let json: serde_json::Value = res.json().await.unwrap_or_default();
    if status >= 400 { Err(friendly(status, &json)) } else { Ok(json) }
}

fn account_from_user(user: &serde_json::Value) -> Option<Account> {
    let email = user.get("email")?.as_str()?.to_string();
    let meta = user.get("user_metadata");
    let text = |k: &str| meta.and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(String::from).filter(|s| !s.is_empty());
    let provider = user.pointer("/app_metadata/provider").and_then(|v| v.as_str()).unwrap_or("email").to_string();
    Some(Account { email, name: text("full_name").or_else(|| text("name")).or_else(|| text("user_name")), avatar: text("avatar_url").or_else(|| text("picture")), provider })
}

/// Guarda los tokens de una respuesta de sesión y devuelve la cuenta.
async fn finish(json: &serde_json::Value) -> Result<Account, String> {
    let tok = |k: &str| json.get(k).and_then(|v| v.as_str()).map(String::from);
    let (Some(access_token), Some(refresh_token)) = (tok("access_token"), tok("refresh_token")) else {
        return Err("El servidor no devolvió una sesión.".into());
    };
    save_session(&Session { access_token, refresh_token }).await?;
    account_from_user(json.get("user").unwrap_or(json)).ok_or_else(|| "El servidor no devolvió los datos de la cuenta.".into())
}

async fn fetch_user(token: &str) -> Result<Option<serde_json::Value>, String> {
    let (url, key) = config().ok_or("sin configurar")?;
    let res = http().get(format!("{url}/auth/v1/user")).header("apikey", key).bearer_auth(token).send().await.map_err(|e| e.to_string())?;
    if res.status().as_u16() == 401 || res.status().as_u16() == 403 {
        return Ok(None);
    }
    Ok(Some(res.json().await.map_err(|e| e.to_string())?))
}

/// La cuenta con sesión iniciada, si la hay. Renueva la sesión si caducó; sin conexión, no cierra la sesión.
pub async fn current() -> Option<Account> {
    config()?;
    let s = load_session().await?;
    match fetch_user(&s.access_token).await {
        Ok(Some(u)) => account_from_user(&u),
        Ok(None) => {
            let json = post("token?grant_type=refresh_token", serde_json::json!({ "refresh_token": s.refresh_token })).await.ok()?;
            finish(&json).await.ok()
        }
        Err(_) => None,
    }
}

pub async fn sign_in_email(email: &str, password: &str) -> Result<Account, String> {
    let json = post("token?grant_type=password", serde_json::json!({ "email": email.trim(), "password": password })).await?;
    finish(&json).await
}

pub async fn sign_up_email(email: &str, password: &str) -> Result<SignUp, String> {
    if password.chars().count() < 8 {
        return Err("La contraseña es demasiado corta o débil. Usa al menos 8 caracteres.".into());
    }
    let json = post("signup", serde_json::json!({ "email": email.trim(), "password": password })).await?;
    if json.get("access_token").is_some() {
        Ok(SignUp::SignedIn { account: finish(&json).await? })
    } else {
        Ok(SignUp::ConfirmEmail)
    }
}

pub async fn sign_out() -> Result<(), String> {
    if let (Some(s), Some((url, key))) = (load_session().await, config()) {
        let _ = http().post(format!("{url}/auth/v1/logout")).header("apikey", key).bearer_auth(s.access_token).send().await;
    }
    tauri::async_runtime::spawn_blocking(|| match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(super::keyring_error(e)),
    })
    .await
    .map_err(|e| e.to_string())?
}

fn random_verifier() -> Result<String, String> {
    use std::io::Read;
    let mut b = [0u8; 48];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).map_err(|_| "No se pudo generar una clave de un solo uso.".to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

fn challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>CoModel</title><body style=\"font-family:system-ui;background:#1a1722;color:#e8e4f0;display:grid;place-items:center;height:100vh;margin:0\"><p>Listo. Ya puedes volver a CoModel y cerrar esta pestaña.</p>";

/// Inicio de sesión con Google o GitHub: abre el navegador del sistema (PKCE) y espera la vuelta en localhost.
pub async fn sign_in_oauth(app: &AppHandle, provider: &str) -> Result<Account, String> {
    if !matches!(provider, "google" | "github") {
        return Err("Proveedor no válido.".into());
    }
    let (url, key) = config().ok_or("El inicio de sesión no está configurado en esta compilación.")?;
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .await
        .map_err(|_| "Otro inicio de sesión ya está en curso. Ciérralo e inténtalo de nuevo.".to_string())?;
    let verifier = random_verifier()?;
    let redirect = format!("http://127.0.0.1:{CALLBACK_PORT}/callback");
    let auth_url = format!(
        "{url}/auth/v1/authorize?provider={provider}&redirect_to={}&code_challenge={}&code_challenge_method=s256",
        urlenc(&redirect),
        challenge(&verifier)
    );
    app.opener().open_url(auth_url, None::<&str>).map_err(|_| "No se pudo abrir el navegador.".to_string())?;

    let code = tokio::time::timeout(Duration::from_secs(300), wait_for_code(&listener))
        .await
        .map_err(|_| "Se agotó el tiempo esperando el inicio de sesión.".to_string())??;
    let _ = key;
    let json = post("token?grant_type=pkce", serde_json::json!({ "auth_code": code, "code_verifier": verifier })).await?;
    finish(&json).await
}

async fn wait_for_code(listener: &TcpListener) -> Result<String, String> {
    loop {
        let (mut sock, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 8192];
        let n = sock.read(&mut buf).await.unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let target = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");
        let query = target.strip_prefix("/callback?");
        let param = |name: &str| query.and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix(&format!("{name}="))).map(String::from));
        let reply = |body: &str| format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        if let Some(code) = param("code") {
            let _ = sock.write_all(reply(DONE_PAGE).as_bytes()).await;
            return Ok(code);
        }
        if param("error").is_some() {
            let _ = sock.write_all(reply("Se canceló el inicio de sesión. Puedes cerrar esta pestaña.").as_bytes()).await;
            return Err("Se canceló el inicio de sesión.".into());
        }
        // Cualquier otra petición (p. ej. el icono de la pestaña) se ignora.
        let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    }
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn user_json_becomes_account() {
        let u = serde_json::json!({"email":"a@b.c","app_metadata":{"provider":"github"},"user_metadata":{"user_name":"ury","avatar_url":"https://x/y.png"}});
        let a = account_from_user(&u).unwrap();
        assert_eq!((a.provider.as_str(), a.name.as_deref()), ("github", Some("ury")));
    }

    #[test]
    fn urlenc_escapes_reserved() {
        assert_eq!(urlenc("http://127.0.0.1:1/callback"), "http%3A%2F%2F127.0.0.1%3A1%2Fcallback");
    }
}
