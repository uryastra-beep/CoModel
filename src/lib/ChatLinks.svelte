<script lang="ts">
  import { canvas, linkedChats, toggleChatLink, type Card } from "./canvas.svelte";
  import Icon from "./Icon.svelte";
  import { popIn, popOut } from "./motion";
  let { card }: { card: Card } = $props();
  let open = $state(false);
  let root: HTMLDivElement;
  const peers = $derived(canvas.cards.filter((c) => c.kind === "chat" && c.id !== card.id));
  const group = $derived(linkedChats(card.id));
  const direct = (id: string) => canvas.links.some((l) => (l.a === card.id && l.b === id) || (l.b === card.id && l.a === id));
  $effect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => { if (!root.contains(e.target as Node)) open = false; };
    const escape = (e: KeyboardEvent) => { if (e.key === "Escape") open = false; };
    window.addEventListener("pointerdown", close, true);
    window.addEventListener("keydown", escape);
    return () => { window.removeEventListener("pointerdown", close, true); window.removeEventListener("keydown", escape); };
  });
</script>
<div class="wrap" bind:this={root}>
  <button class="icon-btn sm" class:connected={group.length > 0} onclick={() => (open = !open)} aria-label="Conectar chats para compartir memoria" title={group.length ? `Memoria compartida con ${group.length} chat(s)` : "Conectar chats para compartir memoria"} aria-expanded={open}><Icon name="plug" size="0.95rem" />{#if group.length}<span>{group.length}</span>{/if}</button>
  {#if open}
    <div class="popover links" in:popIn out:popOut>
      <div class="popover-label">Compartir memoria</div>
      <p>Los chats conectados reciben las notas y los mensajes recientes del grupo al enviar. Usan automáticamente la misma carpeta: la de este chat o, si no tiene, la del otro.</p>
      {#if canvas.projectSync}<p role="status">Compartiendo carpeta…</p>{/if}
      {#each peers as peer}
        <button class="choice" disabled={canvas.projectSync || card.workspacePending || peer.workspacePending || card.busy || peer.busy || (!direct(peer.id) && group.length >= 32)} aria-pressed={direct(peer.id)} onclick={() => toggleChatLink(card.id, peer.id)}>
          <Icon name={direct(peer.id) ? "check" : "plus"} size="0.9rem" /><span>{peer.title}</span><small>{direct(peer.id) ? "Desconectar" : "Conectar"}</small>
        </button>
      {:else}<p>Abre otro chat para conectarlos.</p>{/each}
      {#if group.length}<p>{group.length} chat(s) en el grupo. Las líneas del lienzo muestran las conexiones. Al desconectar dejan de recibir contexto nuevo; lo ya leído puede seguir en su conversación.</p>{/if}
    </div>
  {/if}
</div>
<style>
  .wrap { position: relative; }
  .connected { color: var(--accent); }
  .connected span { font-size: var(--fs-xs); }
  .links { position: absolute; right: 0; top: calc(100% + 0.4rem); width: 20rem; max-height: 25rem; overflow: auto; cursor: default; }
  p { margin: 0.4rem 0.6rem 0.7rem; font-size: var(--fs-xs); color: var(--text-3); line-height: 1.5; }
  .choice { display: flex; gap: 0.5rem; align-items: center; width: 100%; padding: 0.6rem; border: 0; border-radius: var(--radius); background: none; color: var(--text); text-align: left; transition: background-color var(--dur-fast) var(--ease); }
  .choice:hover { background: var(--raised); }
  .choice span { flex: 1; overflow: hidden; text-overflow: ellipsis; }
  small { color: var(--text-3); }
</style>
