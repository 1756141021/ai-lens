<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Settings from "../components/Settings.svelte";
  import { loadConfig, type AppConfig } from "../lib/config";

  let config = $state<AppConfig | null>(null);

  onMount(async () => {
    config = await loadConfig();
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape") getCurrentWindow().hide();
    });
  });

  function close() {
    getCurrentWindow().hide();
  }
</script>

{#if config}
  <Settings {config} onclose={close} />
{:else}
  <div class="loading"></div>
{/if}

<style>
  .loading { height: 100vh; }
</style>
