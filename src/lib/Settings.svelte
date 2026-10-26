<script lang="ts">
  let photoInput = $state<HTMLInputElement>();
  let photoError = $state("");
  async function pickPhoto(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    photoError = "";
    try {
      await setPhoto(input.files?.[0] ?? null);
    } catch (err) {
      photoError = err instanceof Error ? err.message : "No se pudo usar esa imagen.";
    }
    input.value = "";
  }
  import Dialog from "./Dialog.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import { rescaleCards } from "./canvas.svelte";
  import Avatar from "./Avatar.svelte";
  import { photo, profile, setName, setPhoto } from "./profile.svelte";
  import { account, openLogin, signOut } from "./account.svelte";
  import { rise } from "./motion";
  import SettingsAIs from "./SettingsAIs.svelte";
  import SettingsProject from "./SettingsProject.svelte";
  import SettingsUsage from "./SettingsUsage.svelte";
  import { TEXT_SCALES, THEMES, currentTextScale, currentTheme, reducedMotion, setReducedMotion, setTextScale, setTheme, type ThemeId } from "./theme";
  import { closeSettings, ui, type Section } from "./ui.svelte";

  const SECTIONS: { id: Section; label: string; icon: IconName }[] = [
    { id: "perfil", label: "Perfil", icon: "user" },
    { id: "ias", label: "IAs", icon: "plug" },
    { id: "proyecto", label: "Proyectos", icon: "folder" },
    { id: "uso", label: "Uso", icon: "chart" },
    { id: "apariencia", label: "Apariencia", icon: "palette" },
    { id: "atajos", label: "Atajos", icon: "keyboard" },
  ];

  const SHORTCUTS: { group: string; list: { keys: string[]; what: string }[] }[] = [
    {
      group: "General",
      list: [
        { keys: ["Ctrl", "K"], what: "Buscar chats, proyectos y acciones" },
        { keys: ["N"], what: "Nuevo chat" },
        { keys: ["Ctrl", "B"], what: "Mostrar u ocultar la barra lateral" },
        { keys: ["Ctrl", ","], what: "Abrir Ajustes" },
        { keys: ["Esc"], what: "Cerrar un menú o un diálogo" },
      ],
    },
    {
      group: "Lienzo",
      list: [
        { keys: ["Doble clic"], what: "Abrir un chat en ese punto" },
        { keys: ["Rueda"], what: "Desplazar el lienzo" },
        { keys: ["Ctrl", "Rueda"], what: "Acercar o alejar" },
        { keys: ["Arrastrar el fondo"], what: "Desplazar el lienzo" },
        { keys: ["Flechas"], what: "Mover un chat (con su título enfocado); con Mayús, más rápido" },
      ],
    },
    {
      group: "Chat",
      list: [
        { keys: ["Enter"], what: "Enviar el mensaje" },
        { keys: ["Mayús", "Enter"], what: "Salto de línea" },
        { keys: ["/"], what: "Comandos: /compact, /clear, /memory, /help" },
        { keys: ["Ctrl", "S"], what: "Guardar un archivo abierto" },
      ],
    },
  ];

  let theme = $state<string>(currentTheme());
  let scale = $state<string>(currentTextScale());
  let reduce = $state(reducedMotion());
  const current = $derived(SECTIONS.find((s) => s.id === ui.section) ?? SECTIONS[0]);
</script>

<Dialog title="Ajustes" onclose={closeSettings} width="min(60rem, 94vw)" height="min(42rem, 88vh)" bare>
  {#snippet children(close)}
    <div class="body">
      <nav aria-label="Secciones de ajustes">
        <p class="navtitle">Ajustes</p>
        {#each SECTIONS as s (s.id)}
          <button class:current={ui.section === s.id} aria-current={ui.section === s.id ? "page" : undefined} onclick={() => (ui.section = s.id)}>
            <Icon name={s.icon} size="1rem" />{s.label}
          </button>
        {/each}
      </nav>

      <div class="content">
        <button class="icon-btn sm closebtn" onclick={close} aria-label="Cerrar ajustes" title="Cerrar (Esc)"><Icon name="x" size="1rem" /></button>
        {#key ui.section}
          <div class="pane" in:rise={{ y: 6, duration: 170 }}>
            {#if ui.section === "perfil"}
              <h2 class="s-title">Perfil</h2>
              <p class="s-lead">Cómo te llama CoModel y tu cuenta, si quieres usar una.</p>
              <section class="s-group">
                <div class="s-rows">
                  <div class="s-row">
                    <span class="s-row-text">
                      <span class="s-label">Foto</span>
                      <span class="s-desc">Se ve en la barra lateral. Se guarda solo en este equipo.</span>
                      {#if photoError}<span class="s-desc" style:color="var(--danger)" role="alert">{photoError}</span>{/if}
                    </span>
                    <span class="s-control">
                      <Avatar size="2.5rem" />
                      <input bind:this={photoInput} type="file" accept="image/*" class="sr-only" tabindex="-1" onchange={pickPhoto} />
                      <button class="btn" onclick={() => photoInput?.click()}>{photo.src ? "Cambiar" : "Subir foto"}</button>
                      {#if photo.src}<button class="btn ghost" onclick={() => setPhoto(null)}>Quitar</button>{/if}
                    </span>
                  </div>
                  <label class="s-row">
                    <span class="s-row-text">
                      <span class="s-label">Nombre</span>
                      <span class="s-desc">Aparece en el saludo de Inicio. Se guarda solo en este equipo.</span>
                    </span>
                    <span class="s-control"><input class="input" style:width="14rem" value={profile.name} maxlength="30" onchange={(e) => setName(e.currentTarget.value)} /></span>
                  </label>
                  <div class="s-row">
                    <span class="s-row-text">
                      <span class="s-label">Cuenta</span>
                      {#if account.user}
                        <span class="s-desc">Sesión iniciada como {account.user.email} ({account.user.provider === "email" ? "correo" : account.user.provider === "google" ? "Google" : "GitHub"}).</span>
                      {:else}
                        <span class="s-desc">Opcional. Sin cuenta, todo se queda en este equipo.</span>
                      {/if}
                    </span>
                    <span class="s-control">
                      {#if account.user}
                        <button class="btn" onclick={signOut}><Icon name="logout" size="0.95rem" />Cerrar sesión</button>
                      {:else}
                        <button class="btn" onclick={openLogin}>Iniciar sesión</button>
                      {/if}
                    </span>
                  </div>
                </div>
              </section>
            {:else if ui.section === "proyecto"}
              <SettingsProject />
            {:else if ui.section === "ias"}
              <SettingsAIs />
            {:else if ui.section === "uso"}
              <SettingsUsage />
            {:else if ui.section === "apariencia"}
              <h2 class="s-title">Apariencia</h2>
              <p class="s-lead">Tema, tamaño del texto y movimiento. Se aplican al momento.</p>

              <section class="s-group">
                <h3 class="s-group-title">Tema</h3>
                <div class="themes" role="radiogroup" aria-label="Tema">
                  {#each THEMES as t (t.id)}
                    <label class="opt">
                      <input
                        type="radio"
                        name="theme"
                        value={t.id}
                        checked={theme === t.id}
                        onchange={() => {
                          theme = t.id;
                          setTheme(t.id as ThemeId);
                        }}
                      />
                      <span class="swatch" data-theme={t.id} aria-hidden="true">
                        <span class="side"><i></i><i></i><i></i></span>
                        <span class="main">
                          <span class="card"><i></i><i></i></span>
                          <span class="btn-dot"></span>
                        </span>
                      </span>
                      <span class="tname">{t.label}</span>
                      <span class="thint">{t.hint}</span>
                    </label>
                  {/each}
                </div>
              </section>

              <section class="s-group">
                <div class="s-rows">
                  <div class="s-row">
                    <span class="s-row-text">
                      <span class="s-label">Tamaño del texto</span>
                      <span class="s-desc">Escala toda la interfaz, chats incluidos.</span>
                    </span>
                    <span class="s-control">
                      <span class="seg" role="radiogroup" aria-label="Tamaño del texto">
                        {#each TEXT_SCALES as s (s.id)}
                          <label class:on={scale === s.id} title={s.label}>
                            <input
                              type="radio"
                              name="scale"
                              value={s.id}
                              checked={scale === s.id}
                              onchange={() => {
                                rescaleCards(Number(s.id) / Number(scale));
                                scale = s.id;
                                setTextScale(s.id);
                              }}
                            />
                            <span style:font-size="{0.7 + Number(s.id) * 0.25}rem">A</span>
                            <span class="sr-only">{s.label}</span>
                          </label>
                        {/each}
                      </span>
                    </span>
                  </div>
                  <label class="s-row">
                    <span class="s-row-text">
                      <span class="s-label">Reducir movimiento</span>
                      <span class="s-desc">Quita las animaciones. Por defecto sigue la opción de tu sistema.</span>
                    </span>
                    <span class="s-control">
                      <span class="switch">
                        <input
                          type="checkbox"
                          role="switch"
                          checked={reduce}
                          onchange={(e) => {
                            reduce = e.currentTarget.checked;
                            setReducedMotion(reduce);
                          }}
                        />
                        <span class="track" aria-hidden="true"></span>
                      </span>
                    </span>
                  </label>
                </div>
              </section>
            {:else}
              <h2 class="s-title">Atajos</h2>
              <p class="s-lead">Todo lo que se puede hacer sin ratón.</p>
              {#each SHORTCUTS as g (g.group)}
                <section class="s-group">
                  <h3 class="s-group-title">{g.group}</h3>
                  <div class="s-rows">
                    {#each g.list as s}
                      <div class="s-row keyrow">
                        <span class="s-label">{s.what}</span>
                        <span class="keys">{#each s.keys as k, i}{#if i > 0}<span class="plus">+</span>{/if}<kbd>{k}</kbd>{/each}</span>
                      </div>
                    {/each}
                  </div>
                </section>
              {/each}
            {/if}
          </div>
        {/key}
        <span class="sr-only" aria-live="polite">{current.label}</span>
      </div>
    </div>
  {/snippet}
</Dialog>

<style>
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 13rem minmax(0, 1fr);
  }
  nav {
    display: grid;
    align-content: start;
    gap: 0.1rem;
    padding: 1rem 0.65rem;
    border-right: 1px solid var(--line-soft);
    background: color-mix(in oklch, var(--bg) 55%, var(--surface));
    border-radius: var(--radius-xl) 0 0 var(--radius-xl);
  }
  .navtitle {
    margin: 0 0 0.6rem;
    padding: 0 0.55rem;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    height: var(--control);
    padding: 0 0.55rem;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    color: var(--text-2);
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  nav button :global(.icon) {
    color: var(--text-3);
  }
  nav button:hover {
    background: var(--raised);
    color: var(--text);
  }
  nav button.current {
    background: var(--raised-2);
    color: var(--text);
    font-weight: 500;
  }
  nav button.current :global(.icon) {
    color: var(--text);
  }
  .content {
    position: relative;
    min-width: 0;
    overflow-y: auto;
    padding: 1.75rem 2.25rem 2rem;
  }
  .closebtn {
    position: absolute;
    top: 0.85rem;
    right: 0.85rem;
  }
  .pane {
    max-width: 44rem;
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.75rem;
  }
  .opt {
    position: relative;
    display: grid;
    gap: 0.1rem;
    padding: 0.5rem 0.5rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--bg);
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease),
      box-shadow var(--dur-fast) var(--ease);
  }
  .opt:hover {
    border-color: var(--text-3);
  }
  .opt:has(input:checked) {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .opt:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .opt input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .tname {
    margin-top: 0.4rem;
    padding: 0 0.15rem;
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .thint {
    padding: 0 0.15rem;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  /* Vista previa pintada con los tokens del propio tema (data-theme en el elemento). */
  .swatch {
    display: grid;
    grid-template-columns: 28% 1fr;
    height: 5rem;
    border-radius: var(--radius);
    border: 1px solid var(--line-soft);
    background: var(--bg);
    overflow: hidden;
  }
  .side {
    display: grid;
    align-content: start;
    gap: 0.28rem;
    padding: 0.55rem 0.4rem;
    background: var(--surface);
    border-right: 1px solid var(--line-soft);
  }
  .side i {
    display: block;
    height: 0.25rem;
    border-radius: 1rem;
    background: var(--line);
  }
  .side i:first-child {
    background: var(--text-3);
    width: 80%;
  }
  .main {
    position: relative;
    padding: 0.55rem;
    background-image: radial-gradient(var(--dot) 1px, transparent 1.4px);
    background-size: 8px 8px;
  }
  .main .card {
    display: grid;
    gap: 0.28rem;
    width: 70%;
    padding: 0.4rem;
    border-radius: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .main .card i {
    display: block;
    height: 0.22rem;
    border-radius: 1rem;
    background: var(--text-2);
  }
  .main .card i:last-child {
    width: 60%;
    background: var(--text-3);
  }
  .btn-dot {
    position: absolute;
    right: 0.5rem;
    bottom: 0.5rem;
    width: 1.3rem;
    height: 0.6rem;
    border-radius: 0.2rem;
    background: var(--accent-solid);
  }

  .seg {
    display: inline-flex;
    padding: 0.15rem;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .seg label {
    position: relative;
    display: grid;
    place-items: center;
    width: 2.1rem;
    height: 1.7rem;
    border-radius: var(--radius-sm);
    color: var(--text-3);
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .seg label:hover {
    color: var(--text);
  }
  .seg label.on {
    background: var(--raised-2);
    color: var(--text);
  }
  .seg input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .seg label:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .keyrow {
    min-height: 2.6rem;
  }
  .keys {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    flex: none;
  }
  .plus {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
</style>
