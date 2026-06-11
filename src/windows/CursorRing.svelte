<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { loadConfig, type AppConfig } from "../lib/config";

  let color = $state("#ff3b30");
  let opacity = $state(0.4);

  onMount(() => {
    loadConfig().then((c) => {
      color = c.cursor.color;
      opacity = c.cursor.opacity;
    });
    const un = listen<AppConfig>("config-changed", (e) => {
      color = e.payload.cursor.color;
      opacity = e.payload.cursor.opacity;
    });
    return () => {
      un.then((f) => f());
    };
  });
</script>

<div class="ring" style="background:{color}; opacity:{opacity}"></div>

<style>
  :global(html, body) {
    background: transparent;
    overflow: hidden;
  }
  .ring {
    width: 100vw;
    height: 100vh;
    border-radius: 50%;
    pointer-events: none;
  }
</style>
