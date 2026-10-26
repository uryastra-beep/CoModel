<script lang="ts">
  import { accountSignIn, accountSignInOAuth, accountSignUp } from "./backend";
  import Dialog from "./Dialog.svelte";
  import { account, adopt } from "./account.svelte";

  let mode = $state<"in" | "up">("in");
  let email = $state("");
  let password = $state("");
  let busy = $state<"email" | "google" | "github" | null>(null);
  let error = $state("");
  let notice = $state("");

  const valid = $derived(/^\S+@\S+\.\S+$/.test(email.trim()) && password.length > 0);

  async function run(kind: "email" | "google" | "github", close: () => void) {
    if (busy) return;
    busy = kind;
    error = "";
    notice = "";
    try {
      if (kind === "email" && mode === "up") {
        const r = await accountSignUp(email, password);
        if (r.kind === "confirm_email") {
          notice = "Te enviamos un correo. Abre el enlace para confirmar la cuenta y vuelve a entrar.";
          mode = "in";
          password = "";
          return;
        }
        adopt(r.account);
      } else if (kind === "email") {
        adopt(await accountSignIn(email, password));
      } else {
        adopt(await accountSignInOAuth(kind));
      }
      close();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  const label = (kind: "google" | "github", name: string) => (busy === kind ? "Esperando al navegador…" : `Continuar con ${name}`);
</script>

<Dialog title="Iniciar sesión" subtitle="Opcional: CoModel funciona igual sin cuenta." onclose={() => (account.dialog = false)} width="min(25rem, 94vw)">
  {#snippet children(close)}
    <div class="body">
      {#if !account.configured}
        <p class="note">El inicio de sesión no está configurado en esta compilación. CoModel funciona igual sin cuenta.</p>
        <button class="btn ghost skip" onclick={close}>Cerrar</button>
      {:else}
        <p class="lead">Tu cuenta guarda tu nombre y, más adelante, tus ajustes entre equipos.</p>

        <div class="social">
          <button class="btn lg" onclick={() => run("google", close)} disabled={!!busy}>{label("google", "Google")}</button>
          <button class="btn lg" onclick={() => run("github", close)} disabled={!!busy}>{label("github", "GitHub")}</button>
        </div>

        <div class="or" aria-hidden="true"><span>o con tu correo</span></div>

        <form onsubmit={(e) => { e.preventDefault(); if (valid) run("email", close); }}>
          <label>
            <span>Correo</span>
            <input class="input" type="email" bind:value={email} autocomplete="email" required />
          </label>
          <label>
            <span>Contraseña</span>
            <input class="input" type="password" bind:value={password} autocomplete={mode === "in" ? "current-password" : "new-password"} minlength={mode === "up" ? 8 : undefined} required />
            {#if mode === "up"}<small>Al menos 8 caracteres.</small>{/if}
          </label>
          {#if error}<p class="error" role="alert">{error}</p>{/if}
          {#if notice}<p class="notice" role="status">{notice}</p>{/if}
          <button class="btn primary lg" type="submit" disabled={!valid || !!busy}>
            {busy === "email" ? "Un momento…" : mode === "in" ? "Entrar" : "Crear cuenta"}
          </button>
        </form>

        <p class="switch">
          {#if mode === "in"}¿Sin cuenta? <button class="link" onclick={() => { mode = "up"; error = ""; notice = ""; }}>Crear una</button>
          {:else}¿Ya tienes cuenta? <button class="link" onclick={() => { mode = "in"; error = ""; notice = ""; }}>Entrar</button>{/if}
        </p>
        <button class="btn ghost skip" onclick={close}>Ahora no</button>
      {/if}
    </div>
  {/snippet}
</Dialog>

<style>
  .body {
    display: grid;
    gap: 0.9rem;
    padding: 1rem 1.35rem 1.15rem;
  }
  .lead,
  .note {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .social {
    display: grid;
    gap: 0.45rem;
  }
  .or {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .or::before,
  .or::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line-soft);
  }
  form {
    display: grid;
    gap: 0.75rem;
  }
  label {
    display: grid;
    gap: 0.3rem;
  }
  label span {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  small {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .error,
  .notice {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .error {
    color: var(--danger);
  }
  .notice {
    color: var(--text-2);
  }
  .switch {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
    text-align: center;
  }
  .skip {
    justify-self: center;
  }
</style>
