# Cuentas e inicio de sesión

Iniciar sesión es **opcional**: sin cuenta, CoModel funciona igual y todo queda en el equipo. Con cuenta se guarda el nombre (y más adelante los ajustes entre equipos).

Se puede entrar con correo y contraseña, con Google o con GitHub. El servidor de cuentas es [Supabase Auth](https://supabase.com/docs/guides/auth) (código abierto, plan gratuito). CoModel no guarda contraseñas ni tiene servidor propio.

## Cómo funciona

- Correo: `signup` y `token?grant_type=password` de la API de Supabase, llamados desde Rust (`src-tauri/src/account.rs`).
- Google y GitHub: se abre el navegador del sistema, con PKCE. La vuelta llega a `http://127.0.0.1:53682/callback`, un servidor local que vive solo mientras dura el inicio de sesión. No hay secretos de cliente en la app.
- La sesión (tokens) se guarda en el llavero del sistema, igual que las claves de API. Nunca en archivos.
- La URL del proyecto y la clave `anon` son públicas por diseño; la seguridad la dan las reglas del servidor. Pueden ir en el repositorio.

## Configurarlo (una vez, quien publica la app)

1. Crea un proyecto en <https://supabase.com>.
2. En **Project Settings, API**: copia la *Project URL* y la clave *anon public*. Pégalas en `SUPABASE_URL` y `SUPABASE_ANON_KEY` de `src-tauri/src/account.rs` (o usa las variables `COMODEL_SUPABASE_URL` y `COMODEL_SUPABASE_ANON_KEY` al ejecutar).
3. En **Authentication, URL Configuration**: añade `http://127.0.0.1:53682/callback` a *Redirect URLs*.
4. **Correo:** en *Authentication, Providers, Email* déjalo activado. Mantén "Confirm email" activado y la longitud mínima de contraseña en 8.
5. **GitHub:** en GitHub, *Settings, Developer settings, OAuth Apps, New OAuth App*. *Authorization callback URL*: `https://<tu-proyecto>.supabase.co/auth/v1/callback`. Copia *Client ID* y genera un *Client secret*; pégalos en *Authentication, Providers, GitHub* de Supabase (el secreto vive solo en Supabase).
6. **Google:** en <https://console.cloud.google.com>, *APIs y servicios, Credenciales, Crear ID de cliente de OAuth* (tipo "Aplicación web"). *URI de redireccionamiento autorizado*: la misma `https://<tu-proyecto>.supabase.co/auth/v1/callback`. Pega el *Client ID* y el *secreto* en *Authentication, Providers, Google* de Supabase. Configura antes la pantalla de consentimiento (nombre de la app y correo de contacto).
7. Opcional: en *Authentication, SMTP Settings* conecta un correo propio; el de Supabase tiene un límite bajo de envíos.

Sin configurar, el diálogo explica que el inicio de sesión no está disponible y la app sigue funcionando.
