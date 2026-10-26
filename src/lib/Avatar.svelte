<script lang="ts">
  // Foto de perfil o, si no hay, la inicial del nombre.
  import { photo, profile } from "./profile.svelte";
  let { size = "1.75rem" }: { size?: string } = $props();
</script>

<span class="avatar" style:width={size} style:height={size} style:font-size="calc({size} * 0.4)" aria-hidden="true">
  {#if photo.src}<img src={photo.src} alt="" />{:else}{profile.name.charAt(0).toUpperCase()}{/if}
</span>

<style>
  .avatar {
    display: grid;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: 50%;
    background: var(--raised-2);
    border: 1px solid var(--line);
    color: var(--text);
    font-weight: 600;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
