<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import Settings from "../components/Settings.svelte";
  import { loadConfig, type AppConfig } from "../lib/config";

  let config = $state<AppConfig | null>(null);

  onMount(() => {
    loadConfig().then((c) => (config = c));
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape") getCurrentWindow().hide();
    });
    // The window is hidden and reused, not destroyed — without this the form
    // shows stale values after the overlay persists a model switch.
    const unlisten = listen<AppConfig>("config-changed", (e) => {
      config = e.payload;
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  function close() {
    getCurrentWindow().hide();
  }
</script>

{#if config}
  {#key config}
    <Settings {config} onclose={close} />
  {/key}
{:else}
  <div class="loading"></div>
{/if}

<style>
  .loading { height: 100vh; }
</style>
