//! OAuth público de ChatGPT: PKCE, identidad verificada y credenciales únicamente en el llavero.
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{rand::{SecureRandom, SystemRandom}, signature};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, sync::{Mutex, Notify}};

const ISSUER: &str = "https://auth.openai.com";
const RESOURCE: &str = "https://api.openai.com/v1";
const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
const SCOPE: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
static AUTH: Mutex<()> = Mutex::const_new(());
static LOGIN: Mutex<()> = Mutex::const_new(());
static CANCEL: Notify = Notify::const_new();

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    client_id: String, subject: String, email: String,
    access_token: Option<String>, refresh_token: Option<String>, id_token: Option<String>,
    expires_at: u64, scopes: Vec<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct Store { active: Option<String>, sessions: Vec<Session> }
#[derive(Serialize)]
pub struct Account { pub id: String, pub label: String, pub active: bool, pub connected: bool }
#[derive(Clone, Serialize, Deserialize)]
pub struct Model { pub id: String, pub name: String }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn random() -> Result<String, String> { let mut b = [0u8;32]; SystemRandom::new().fill(&mut b).map_err(|_| "No se pudo preparar el inicio de sesión.")?; Ok(URL_SAFE_NO_PAD.encode(b)) }
fn http() -> Result<reqwest::Client, String> { reqwest::Client::builder().timeout(Duration::from_secs(25)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "No se pudo preparar la conexión con OpenAI.".into()) }
fn entry() -> Result<keyring::Entry, String> { keyring::Entry::new("comodel", "chatgpt-sessions").map_err(super::keyring_error) }
async fn load() -> Result<Store, String> {
    tauri::async_runtime::spawn_blocking(|| match entry()?.get_password() {
        Ok(v) => serde_json::from_str(&v).map_err(|_| "La conexión guardada de ChatGPT no se puede leer.".into()),
        Err(keyring::Error::NoEntry) => Ok(Store::default()), Err(e) => Err(super::keyring_error(e)),
    }).await.map_err(|_| "No se pudo leer la conexión de ChatGPT.".to_string())?
}
async fn save(store: &Store) -> Result<(), String> {
    let value = serde_json::to_string(store).map_err(|_| "No se pudo guardar la conexión de ChatGPT.")?;
    tauri::async_runtime::spawn_blocking(move || entry()?.set_password(&value).map_err(super::keyring_error)).await.map_err(|_| "No se pudo guardar la conexión de ChatGPT.".to_string())?
}
fn ready(s: &Session) -> bool { s.access_token.is_some() && s.scopes.iter().any(|s| s == "chatgpt.tokens.use.direct") }
pub async fn accounts() -> Result<Vec<Account>, String> {
    let _guard = AUTH.lock().await;
    let s = load().await?;
    Ok(s.sessions.iter().map(|p| Account { id:p.client_id.clone(), label:format!("{} · {}", p.email, p.client_id.chars().rev().take(6).collect::<String>().chars().rev().collect::<String>()), active:s.active.as_ref()==Some(&p.client_id), connected:ready(p) }).collect())
}
pub async fn status() -> (bool, String) {
    match accounts().await { Ok(list) => list.into_iter().find(|a| a.active).map(|a| (a.connected,a.label)).unwrap_or((false,"Inicia sesión con ChatGPT y permite usar tu plan.".into())), Err(e) => (false,e) }
}
pub async fn select(id: String) -> Result<(), String> {
    let _guard = AUTH.lock().await; let mut s=load().await?;
    if !s.sessions.iter().any(|a| a.client_id==id && ready(a)) { return Err("Vuelve a iniciar sesión en esa cuenta de ChatGPT.".into()); }
    s.active=Some(id);save(&s).await
}
pub fn cancel() { CANCEL.notify_waiters(); }
async fn host(app: &AppHandle) -> Result<String,String> {
    let path=app.path().app_config_dir().map_err(|_| "No se encontró la configuración de CoModel.")?.join("chatgpt-host-id");
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok(s)=std::fs::read_to_string(&path) {
            let id=s.trim();
            let uuid=id.strip_prefix("urn:uuid:").is_some_and(|u| {
                u.len()==36 && u.bytes().enumerate().all(|(i,c)| {
                    if matches!(i,8|13|18|23) { c==b'-' } else { c.is_ascii_hexdigit() }
                }) && u.as_bytes()[14]==b'4' && matches!(u.as_bytes()[19],b'8'|b'9'|b'a'|b'b'|b'A'|b'B')
            });
            if uuid || id.starts_with("urn:ietf:params:oauth:jwk-thumbprint:") || id.starts_with("did:key:") {
                return Ok(id.to_string());
            }
        }
        // UUIDv4 en formato URI, aceptado por SIWC. Migra el antiguo comodel-… inválido una sola vez.
        let mut bytes=[0u8;16];
        SystemRandom::new().fill(&mut bytes).map_err(|_| "No se pudo generar el identificador de CoModel.")?;
        bytes[6]=(bytes[6]&0x0f)|0x40;
        bytes[8]=(bytes[8]&0x3f)|0x80;
        let hex=bytes.iter().map(|b|format!("{b:02x}")).collect::<String>();
        let id=format!("urn:uuid:{}-{}-{}-{}-{}",&hex[..8],&hex[8..12],&hex[12..16],&hex[16..20],&hex[20..]);
        if let Some(p)=path.parent() { std::fs::create_dir_all(p).map_err(|_| "No se pudo guardar el identificador de CoModel.")?; }
        let temp=path.with_extension("tmp");
        std::fs::write(&temp,&id).map_err(|_| "No se pudo guardar el identificador de CoModel.")?;
        std::fs::rename(&temp,&path).map_err(|_| "No se pudo guardar el identificador de CoModel.")?;
        Ok(id)
    }).await.map_err(|_| "No se pudo preparar CoModel.".to_string())?
}

async fn identity(jwt:&str, client_id:&str, nonce:&str) -> Result<Value,String> {
    let parts:Vec<_>=jwt.split('.').collect();
    if parts.len()!=3 { return Err("OpenAI devolvió una identidad no válida.".into()); }
    let decode=|s:&str| URL_SAFE_NO_PAD.decode(s).map_err(|_| "OpenAI devolvió una identidad no válida.".to_string());
    let header:Value=serde_json::from_slice(&decode(parts[0])?).map_err(|_| "Cabecera de identidad no válida.")?;
    let claims:Value=serde_json::from_slice(&decode(parts[1])?).map_err(|_| "Identidad no válida.")?;
    let kid=header["kid"].as_str().ok_or("La identidad de OpenAI no tiene clave de firma.")?;
    let res=http()?.get(format!("{ISSUER}/.well-known/jwks.json")).send().await.map_err(|_| "No se pudo verificar la identidad de OpenAI.")?;
    if !res.status().is_success() { return Err("No se pudo obtener la firma de OpenAI.".into()); }
    let keys:Value=res.json().await.map_err(|_| "OpenAI devolvió claves no válidas.")?;
    let key=keys["keys"].as_array().and_then(|ks|ks.iter().find(|k|k["kid"]==kid)).ok_or("No se encontró la clave de firma de OpenAI.")?;
    let signature=decode(parts[2])?;let signed=format!("{}.{}",parts[0],parts[1]);
    let verified=match header["alg"].as_str() {
        Some("RS256") if key["kty"]=="RSA" => {
            let n=decode(key["n"].as_str().ok_or("Clave RSA no válida.")?)?;let e=decode(key["e"].as_str().ok_or("Clave RSA no válida.")?)?;
            signature::RsaPublicKeyComponents{n:&n,e:&e}.verify(&signature::RSA_PKCS1_2048_8192_SHA256,signed.as_bytes(),&signature)
        }
        Some("ES256") if key["kty"]=="EC" && key["crv"]=="P-256" => {
            let mut public=vec![4];public.extend(decode(key["x"].as_str().ok_or("Clave EC no válida.")?)?);public.extend(decode(key["y"].as_str().ok_or("Clave EC no válida.")?)?);
            signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED,public).verify(signed.as_bytes(),&signature)
        }
        _ => return Err("OpenAI devolvió un algoritmo de firma no admitido.".into()),
    };
    verified.map_err(|_| "La firma de la identidad de OpenAI no es válida.")?;
    let audience=claims["aud"].as_str()==Some(client_id)||claims["aud"].as_array().is_some_and(|a|a.iter().any(|v|v==client_id));
    if claims["iss"]!=ISSUER || !audience || claims["exp"].as_u64().unwrap_or(0)<=now() || claims["iat"].as_u64().is_none_or(|t|t>now()+30) || claims["nonce"]!=nonce || claims["sub"].as_str().is_none_or(str::is_empty) { return Err("No se pudo validar la identidad de ChatGPT. Repite el inicio de sesión.".into()); }
    Ok(claims)
}

pub async fn sign_in(app:AppHandle, account_id:Option<String>) -> Result<String,String> {
    let _login=LOGIN.try_lock().map_err(|_| "Ya hay un inicio de sesión de ChatGPT en curso.")?;
    let old={let _guard=AUTH.lock().await;load().await?.sessions.into_iter().find(|s|Some(&s.client_id)==account_id.as_ref())};
    if account_id.is_some() && old.is_none() { return Err("No se encontró la cuenta de ChatGPT seleccionada.".into()); }
    let listener=TcpListener::bind("127.0.0.1:0").await.map_err(|_| "No se pudo preparar la vuelta del navegador.")?;
    let redirect=format!("http://127.0.0.1:{}/auth/callback",listener.local_addr().map_err(|_| "No se pudo preparar la conexión local.")?.port());
    let state=random()?;let nonce=random()?;let verifier=random()?;let challenge=URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));let host_id=host(&app).await?;
    let mut url=reqwest::Url::parse(&format!("{ISSUER}/api/accounts/authorize")).map_err(|_| "URL de OpenAI no válida.")?;
    {let mut q=url.query_pairs_mut();q.append_pair("client_id",old.as_ref().map(|s|s.client_id.as_str()).unwrap_or("dynamic_agent_client")).append_pair("ext_agent_host_id",&host_id).append_pair("response_type","code").append_pair("redirect_uri",&redirect).append_pair("scope",SCOPE).append_pair("resource",RESOURCE).append_pair("state",&state).append_pair("nonce",&nonce).append_pair("code_challenge_method","S256").append_pair("code_challenge",&challenge);
    if old.is_none(){q.append_pair("agent_name_hint","CoModel");}}
    app.opener().open_url(url.as_str(),None::<&str>).map_err(|_| "No se pudo abrir el navegador para conectar ChatGPT.")?;
    let callback=async {
        loop {
            let (mut socket,_)=listener.accept().await.map_err(|_| "No se pudo recibir la vuelta de ChatGPT.")?;
            let read=async {
                let mut bytes=Vec::new();let mut chunk=[0u8;1024];
                while bytes.len()<16384 { let n=socket.read(&mut chunk).await.map_err(|_| "No se pudo leer la vuelta de ChatGPT.")?;if n==0{break;}bytes.extend_from_slice(&chunk[..n]);if bytes.windows(4).any(|w|w==b"\r\n\r\n"){break;} }
                Ok::<_,String>(bytes)
            };
            let bytes=match tokio::time::timeout(Duration::from_secs(5),read).await {Ok(Ok(b))=>b,_=>continue};
            let request=String::from_utf8_lossy(&bytes);let target=request.lines().next().and_then(|l|l.strip_prefix("GET ")).and_then(|l|l.split(' ').next()).unwrap_or("");
            let parsed=reqwest::Url::parse(&format!("http://127.0.0.1{target}"));
            let params=parsed.as_ref().ok().filter(|u|u.path()=="/auth/callback").map(|u|u.query_pairs().into_owned().collect::<std::collections::HashMap<_,_>>());
            let valid=params.as_ref().and_then(|p|p.get("state"))==Some(&state);
            let body=if valid{"CoModel recibió la autorización. Puedes volver a la app."}else{"Esta solicitud no corresponde al inicio de sesión de CoModel."};
            let reply=format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",if valid{"200 OK"}else{"400 Bad Request"},body.len());let _=socket.write_all(reply.as_bytes()).await;
            if valid {return Ok::<_,String>(params.unwrap());}
        }
    };
    let params=tokio::select! { result=tokio::time::timeout(Duration::from_secs(300),callback)=>result.map_err(|_|"El inicio de sesión de ChatGPT caducó. Inténtalo de nuevo.")??, _=CANCEL.notified()=>return Err("Inicio de sesión cancelado.".into()) };
    if params.contains_key("error"){return Err("No se autorizó la conexión. Debes permitir que CoModel use tu plan de ChatGPT.".into());}
    let code=params.get("code").ok_or("La autorización no devolvió un código.")?;
    let client_id=match &old {Some(s)=>{if params.get("client_id").is_some_and(|id|id!=&s.client_id){return Err("La autorización pertenece a otra cuenta.".into());}s.client_id.clone()},None=>params.get("client_id").filter(|id|id.as_str()!="dynamic_agent_client").cloned().ok_or("OpenAI no completó el registro de CoModel.")?};
    let response=http()?.post(TOKEN).form(&[("grant_type","authorization_code"),("client_id",&client_id),("code",code),("code_verifier",&verifier),("redirect_uri",&redirect),("resource",RESOURCE)]).send().await.map_err(|_| "No se pudo completar la autorización con OpenAI.")?;
    if !response.status().is_success(){return Err("OpenAI no completó el inicio de sesión. Repite la conexión desde CoModel.".into());}
    let token:Value=response.json().await.map_err(|_| "OpenAI devolvió una autorización que no se entiende.")?;
    let id_token=token["id_token"].as_str().ok_or("OpenAI no devolvió una identidad verificable.")?;
    let claims=identity(id_token,&client_id,&nonce).await?;
    if old.as_ref().is_some_and(|s|s.subject!=claims["sub"].as_str().unwrap_or("")){return Err("La cuenta autorizada es diferente de la seleccionada.".into());}
    let scopes:Vec<String>=token["scope"].as_str().unwrap_or("").split_whitespace().map(String::from).collect();
    if !scopes.iter().any(|s|s=="chatgpt.tokens.use.direct"){return Err("La cuenta se identificó, pero no permitió usar su plan de ChatGPT. Conéctala de nuevo y concede ese permiso.".into());}
    if token["token_type"].as_str().is_none_or(|s|!s.eq_ignore_ascii_case("Bearer")){return Err("OpenAI devolvió un tipo de autorización no válido.".into());}
    let session=Session{client_id:client_id.clone(),subject:claims["sub"].as_str().unwrap().into(),email:claims["email"].as_str().unwrap_or("Cuenta de ChatGPT").into(),access_token:Some(token["access_token"].as_str().ok_or("OpenAI no devolvió acceso al plan.")?.into()),refresh_token:token["refresh_token"].as_str().map(String::from),id_token:Some(id_token.into()),expires_at:now()+token["expires_in"].as_u64().unwrap_or(3600),scopes};
    let label=session.email.clone();let _guard=AUTH.lock().await;let mut store=load().await?;store.sessions.retain(|s|s.client_id!=client_id);store.sessions.push(session);store.active=Some(client_id);save(&store).await?;
    Ok(format!("ChatGPT conectado: {label}"))
}

pub async fn access_token() -> Result<String,String> {
    let _guard=AUTH.lock().await;let mut store=load().await?;let active=store.active.clone().ok_or("Conecta ChatGPT en Ajustes → IAs.")?;
    let session=store.sessions.iter_mut().find(|s|s.client_id==active).ok_or("No se encontró la cuenta activa de ChatGPT.")?;
    if !ready(session){return Err("Conecta ChatGPT y permite usar tu plan en Ajustes → IAs.".into());}
    if session.expires_at>now()+90{return session.access_token.clone().ok_or("Vuelve a conectar ChatGPT.".into());}
    let refresh=session.refresh_token.as_deref().ok_or("La sesión de ChatGPT caducó. Vuelve a conectarla.")?;
    let response=http()?.post(TOKEN).form(&[("grant_type","refresh_token"),("client_id",session.client_id.as_str()),("refresh_token",refresh),("resource",RESOURCE)]).send().await.map_err(|_| "No se pudo renovar la sesión de ChatGPT. Revisa tu conexión.")?;
    if !response.status().is_success(){return Err("OpenAI no renovó la sesión. Vuelve a conectar ChatGPT en Ajustes → IAs.".into());}
    let value:Value=response.json().await.map_err(|_| "No se pudo leer la renovación de ChatGPT.")?;
    session.access_token=Some(value["access_token"].as_str().ok_or("OpenAI no renovó el acceso a ChatGPT.")?.into());
    if let Some(t)=value["refresh_token"].as_str(){session.refresh_token=Some(t.into());}
    if let Some(s)=value["scope"].as_str(){session.scopes=s.split_whitespace().map(String::from).collect();}
    if !ready(session){return Err("La sesión ya no tiene permiso para usar el plan. Conecta ChatGPT de nuevo.".into());}
    session.expires_at=now()+value["expires_in"].as_u64().unwrap_or(3600);let token=session.access_token.clone().unwrap();save(&store).await?;Ok(token)
}

pub async fn models() -> Result<Vec<Model>,String> {
    let token=access_token().await?; models_for_token(&token).await
}

async fn models_for_token(token: &str) -> Result<Vec<Model>,String> {
    let response=http()?.get(format!("{RESOURCE}/models")).bearer_auth(token).send().await.map_err(|_| "No se pudo cargar la lista de modelos de ChatGPT.")?;
    if !response.status().is_success(){return Err("ChatGPT rechazó la consulta de modelos. Revisa tu cuenta y el permiso para usar tu plan.".into());}
    let value:Value=response.json().await.map_err(|_| "ChatGPT devolvió una lista de modelos no válida.")?;
    let list=value["models"].as_array().ok_or("ChatGPT no devolvió su catálogo de modelos.")?.iter().filter(|m|m["visibility"]=="list").filter_map(|m|Some(Model{id:m["slug"].as_str()?.into(),name:m["display_name"].as_str()?.into()})).collect::<Vec<_>>();
    if list.is_empty(){return Err("Esta cuenta no ofrece modelos para CoModel. Comprueba la elegibilidad de tu plan en ChatGPT.".into());}Ok(list)
}

pub async fn sign_out() -> Result<String,String> {
    let _guard=AUTH.lock().await;let mut store=load().await?;let active=store.active.clone().ok_or("No hay una cuenta activa de ChatGPT.")?;let s=store.sessions.iter_mut().find(|s|s.client_id==active).ok_or("No se encontró la cuenta de ChatGPT.")?;
    let mut revoked=false;
    if let Some(refresh)=&s.refresh_token {
        if let Ok(client)=http(){if let Ok(discovery)=client.get(format!("{ISSUER}/.well-known/openid-configuration")).send().await{if let Ok(v)=discovery.json::<Value>().await{if let Some(endpoint)=v["revocation_endpoint"].as_str().filter(|s|reqwest::Url::parse(s).ok().is_some_and(|u|u.scheme()=="https"&&u.host_str()==Some("auth.openai.com"))){if let Ok(r)=client.post(endpoint).form(&[("token",refresh.as_str()),("token_type_hint","refresh_token"),("client_id",s.client_id.as_str())]).send().await{revoked=r.status().is_success();}}}}}
    }
    s.access_token=None;s.refresh_token=None;s.id_token=None;s.expires_at=0;store.active=None;save(&store).await?;
    Ok(if revoked{"Sesión de ChatGPT cerrada.".into()}else{"Sesión cerrada en CoModel. No se pudo confirmar la revocación remota; puedes desconectar CoModel desde los ajustes de ChatGPT.".into()})
}

/// El token se renueva por turno, aunque el orquestador lleve horas abierto.
pub struct ChatGPTProvider;
#[async_trait::async_trait]
impl comodel_core::provider::Provider for ChatGPTProvider {
    fn id(&self) -> &str { "chatgpt" }
    fn reads_files(&self) -> bool { false }
    fn native_attachments(&self) -> bool { true }
    async fn run_turn(&self, req: comodel_core::provider::TurnRequest, tx: tokio::sync::mpsc::Sender<comodel_core::events::ProviderEvent>) -> Result<(), comodel_core::provider::ProviderError> {
        let token = match access_token().await { Ok(t) => t, Err(e) => { let _ = tx.send(comodel_core::events::ProviderEvent::Error(e)).await; return Ok(()); } };
        let list = match models_for_token(&token).await { Ok(l) => l, Err(e) => { let _ = tx.send(comodel_core::events::ProviderEvent::Error(e)).await; return Ok(()); } };
        let model = if let Some(chosen) = req.model.as_ref().filter(|s| !s.is_empty()) {
            match list.iter().find(|m| &m.id == chosen) { Some(m) => m.id.clone(), None => { let _ = tx.send(comodel_core::events::ProviderEvent::Error("El modelo elegido ya no está disponible para esta cuenta. Elige otro en el selector de ChatGPT.".into())).await; return Ok(()); } }
        } else { list[0].id.clone() };
        comodel_core::chatgpt::ChatGPT { access_token: token, model }.run_turn(req, tx).await
    }
}
