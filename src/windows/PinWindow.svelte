<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { PhysicalSize } from "@tauri-apps/api/dpi";

  let src = $state("");
  let w0 = 0, h0 = 0;
  let scale = 1;
  const MIN = 0.25, MAX = 3, STEP = 1.1;

  onMount(() => {
    const win = getCurrentWindow();
    (async () => {
      try {
        const r = await invoke<{ base64: string; width: number; height: number }>("get_pin_image");
        w0 = r.width;
        h0 = r.height;
        src = `data:image/png;base64,${r.base64}`;
      } catch {
        win.close(); // stale label, nothing to show
      }
    })();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") win.close();
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      scale = Math.min(MAX, Math.max(MIN, scale * (e.deltaY < 0 ? STEP : 1 / STEP)));
      win.setSize(new PhysicalSize(Math.max(1, Math.round(w0 * scale)), Math.max(1, Math.round(h0 * scale))));
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("wheel", onWheel);
    };
  });

  // Built hidden — show only once the bitmap is painted (no background flash).
  async function onLoad() {
    const win = getCurrentWindow();
    await win.show();
    await win.setFocus();
  }
</script>

<!-- "deep": any descendant drags the window; BUTTONs block dragging natively -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pin" data-tauri-drag-region="deep" oncontextmenu={(e) => e.preventDefault()}>
  {#if src}<img {src} alt="" draggable="false" onload={onLoad} />{/if}
  <button class="x" title="关闭 (Esc)" onclick={() => getCurrentWindow().close()} aria-label="关闭">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round">
      <line x1="6" y1="6" x2="18" y2="18"/><line x1="18" y1="6" x2="6" y2="18"/>
    </svg>
  </button>
</div>

<style>
  :global(html, body) {
    background: transparent;
  }
  .pin {
    position: fixed;
    inset: 0;
    overflow: hidden;
    /* outline, not border — border + box-sizing would shrink the img and break 1:1 */
    outline: 1px solid rgba(255, 255, 255, 0.18);
    outline-offset: -1px;
  }
  img {
    width: 100vw;
    height: 100vh;
    display: block;
    user-select: none;
    -webkit-user-drag: none;
  }
  .x {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 6px;
    background: rgba(24, 26, 33, 0.72);
    color: #cdd2dc;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.12s;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .pin:hover .x {
    opacity: 1;
  }
  .x:hover {
    background: rgba(24, 26, 33, 0.92);
    color: #fff;
  }
  .x svg {
    width: 11px;
    height: 11px;
  }
</style>
