<script lang="ts">
  import { onMount } from "svelte";
  import { chatgptAccounts, chatgptCancelSignIn, chatgptSelectAccount, chatgptSignIn, chatgptSignOut, type ChatGPTAccount } from "./backend";
  import { loadCatalog, resetChatGPTCatalog } from "./ai.svelte";
  import { refreshConnections } from "./connections.svelte";
  import Icon from "./Icon.svelte";
  let accounts = $state<ChatGPTAccount[]>([]);
  let busy = $state(false);
  let signingIn = $state(false);
  let message = $state("");
  let failed = $state(false);
  const active = $derived(accounts.find((a) => a.active));
  onMount(() => { chatgptAccounts().then((a) => (accounts = a)).catch((e) => { message = String(e); failed = true; }); });
  async function run(action: () => Promise<string | void>) {
    if (busy) return;
    busy = true; message = ""; failed = false;
    try {
      message = (await action()) ?? "Cuenta de ChatGPT seleccionada.";
      resetChatGPTCatalog();
      accounts = await chatgptAccounts();
      if (accounts.some((a) => a.active && a.connected)) await loadCatalog("chatgpt");
    } catch (e) { message = String(e); failed = true; }
    finally { signingIn = false; busy = false; await refreshConnections(); }
  }
  function login(id: string | null = null) { signingIn = true; return run(() => chatgptSignIn(id)); }
</script>
<div class="connection">
  {#if accounts.length}
    <label for="chatgpt-account">Cuenta de ChatGPT</label>
    <select id="chatgpt-account" class="input" value={active?.id ?? ""} disabled={busy} onchange={(e) => {
      const id=e.currentTarget.value;
      if (!id) return;
      const account=accounts.find((a) => a.id===id);
      if (account?.connected) run(() => chatgptSelectAccount(id)); else login(id);
    }}>
      <option value="" disabled>Selecciona una cuenta</option>
      {#each accounts as account}<option value={account.id}>{account.label}{account.connected ? "" : " · volver a conectar"}</option>{/each}
    </select>
  {/if}
  <div class="actions">
    {#if signingIn}
      <span role="status">Completa la autorización en el navegador…</span>
      <button class="btn sm" onclick={chatgptCancelSignIn}>Cancelar</button>
    {:else}
      <button class="btn primary sm" disabled={busy} onclick={() => login()}>{active?.connected ? "Añadir otra cuenta de ChatGPT" : "Continuar con ChatGPT"}</button>
      {#if active?.connected}<button class="btn ghost sm" disabled={busy} onclick={() => run(chatgptSignOut)}>Desconectar cuenta</button>{/if}
    {/if}
  </div>
  <p class="note"><Icon name="shield" size="0.8rem" />La autorización se guarda en el llavero del sistema. Usa la cuota de tu plan elegible; tus chats y la memoria de ChatGPT no se importan automáticamente.</p>
  <p class="note">Adjunta PNG, JPEG, WebP, GIF, PDF, Word, presentaciones, hojas de cálculo o texto. Hasta 20 MB por archivo y 50 MB en la petición. Los documentos distintos de PDF se analizan como texto; audio y vídeo no están admitidos.</p>
  {#if message}<p class="s-result" class:bad={failed} role="status">{message}</p>{/if}
</div>
<style>
  .connection { margin-top: 0.7rem; max-width: 38rem; display: grid; gap: 0.5rem; }
  label { font-size: var(--fs-xs); font-weight: 600; color: var(--text-2); }
  select { width: 100%; }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; }
  .actions span { font-size: var(--fs-sm); color: var(--text-2); }
  .note { margin: 0; display: flex; align-items: flex-start; gap: 0.4rem; color: var(--text-3); font-size: var(--fs-xs); line-height: 1.5; }
  .note :global(.icon) { flex: none; margin-top: 0.15rem; }
</style>
