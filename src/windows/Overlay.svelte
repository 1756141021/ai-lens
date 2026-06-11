<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { Marked } from "marked";
  import hljs from "highlight.js";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { loadConfig, saveConfig, toApiConfig, type AppConfig } from "../lib/config";
  import { fetchModels, type ApiConfig } from "../lib/api";
  import {
    getMessages,
    isStreaming,
    getError,
    sendMessage,
    cancelStream,
    clearChat,
  } from "../lib/chat.svelte";

  type Meta = {
    width: number;
    height: number;
    originX: number;
    originY: number;
    scale: number;
  };

  let config: AppConfig | null = null;
  let apiConfig: ApiConfig | null = null;

  let canvas: HTMLCanvasElement;
  let dpr = 1;

  let phase = $state<"select" | "ask">("select");
  let dragging = false;
  let sx = 0, sy = 0, ex = 0, ey = 0;
  let selRect = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let region = { x: 0, y: 0, w: 0, h: 0 };

  let imageBase64: string | undefined;
  let ocrText: string | undefined;
  let capturePromise: Promise<void> | null = null;
  let captureError = $state<string | null>(null);
  let text = $state("");
  let inputEl = $state<HTMLInputElement>();
  let msgsEl = $state<HTMLDivElement>();
  let pendingSend = $state(false);
  let copiedIdx = $state(-1);

  // --- model switch (ask bar) ---
  let curModel = $state("");
  let modelList = $state<string[]>([]);
  let menuOpen = $state(false);
  let pullingModels = $state(false);
  let pullError = $state<string | null>(null);

  const PRESETS = [
    { label: "解释", text: "解释这块内容" },
    { label: "翻译", text: "翻译这块内容：外语翻译成中文，中文翻译成英文" },
    { label: "总结", text: "用几句话总结这块内容的要点" },
  ];

  // --- annotation (drawn in CSS px, same space as selRect) ---
  type Tool = "none" | "arrow" | "rect" | "pen";
  type Shape =
    | { tool: "arrow" | "rect"; color: string; x1: number; y1: number; x2: number; y2: number }
    | { tool: "pen"; color: string; points: { x: number; y: number }[] };
  const COLORS = ["#ff3b30", "#ffcc00", "#34c759", "#3a82f6", "#ffffff"];
  let tool = $state<Tool>("none");
  let color = $state(COLORS[0]);
  let shapes = $state<Shape[]>([]);
  let cur: Shape | null = null; // in-progress stroke
  let drawingAnno = false;

  const COPY_SVG = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/></svg>`;
  const CHECK_SVG = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 12 10 18 20 6"/></svg>`;
  const FAIL_SVG = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><line x1="6" y1="6" x2="18" y2="18"/><line x1="18" y1="6" x2="6" y2="18"/></svg>`;

  const marked = new Marked({
    renderer: {
      code({ text, lang }) {
        const language = lang && hljs.getLanguage(lang) ? lang : "plaintext";
        return `<div class="cb"><button class="cbcopy" title="复制代码">${COPY_SVG}</button><pre><code class="hljs">${hljs.highlight(text, { language }).value}</code></pre></div>`;
      },
    },
  });
  const renderMd = (c: string) => marked.parse(c) as string;

  async function copyText(t: string): Promise<boolean> {
    try {
      await writeText(t);
      return true;
    } catch {
      return false;
    }
  }

  // Code-block buttons live inside {@html} markup, so Svelte can't bind them —
  // one delegated handler on .msgs covers every block.
  async function onMsgsClick(e: MouseEvent) {
    const btn = (e.target as Element).closest?.(".cbcopy") as HTMLButtonElement | null;
    if (!btn) return;
    const code = btn.parentElement?.querySelector("pre code");
    const ok = await copyText(code?.textContent ?? "");
    btn.innerHTML = ok ? CHECK_SVG : FAIL_SVG;
    setTimeout(() => {
      if (btn.isConnected) btn.innerHTML = COPY_SVG;
    }, 1200);
  }

  async function copyAnswer(i: number, content: string) {
    if (!(await copyText(content))) return;
    copiedIdx = i;
    setTimeout(() => {
      if (copiedIdx === i) copiedIdx = -1;
    }, 1200);
  }

  onMount(() => {
    const ro = new ResizeObserver(() => sizeCanvas());
    ro.observe(document.documentElement);
    window.addEventListener("keydown", onKey);
    // Reused (prewarmed) overlay: every capture re-inits via this event.
    const unlisten = listen("capture-ready", () => initCapture());
    // First mount: prebuilt + hidden → meta is null → stays hidden until a real capture.
    initCapture();
    return () => {
      ro.disconnect();
      window.removeEventListener("keydown", onKey);
      unlisten.then((u) => u());
    };
  });

  async function initCapture() {
    // Reload config each capture so provider/model changes take effect.
    try {
      config = await loadConfig();
      apiConfig = toApiConfig(config);
      curModel = config.api.model;
      modelList = config.api.models ?? [];
    } catch {}

    const m = await invoke<Meta | null>("get_capture_meta");
    if (!m) return; // prewarmed + hidden, nothing to show yet

    // Reset per-capture state — miss one and the next capture carries stale data.
    clearChat();
    phase = "select";
    selRect = null;
    dragging = false;
    imageBase64 = undefined;
    ocrText = undefined;
    capturePromise = null;
    captureError = null;
    text = "";
    tool = "none";
    shapes = [];
    cur = null;
    drawingAnno = false;
    pendingSend = false;
    copiedIdx = -1;
    menuOpen = false;
    pullingModels = false;
    pullError = null;

    dpr = window.devicePixelRatio || 1;
    sizeCanvas();
    draw();
    // Transparent overlay over the live desktop — nothing to decode, show now.
    const w = getCurrentWindow();
    await w.show();
    await w.setFocus();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (menuOpen) {
        menuOpen = false;
        inputEl?.focus();
        return;
      }
      getCurrentWindow().hide();
    }
  }

  function sizeCanvas() {
    if (!canvas) return;
    canvas.width = window.innerWidth * dpr;
    canvas.height = window.innerHeight * dpr;
    draw();
  }

  function rectPx(): { x: number; y: number; w: number; h: number } | null {
    if (phase === "ask" && selRect) {
      return { x: selRect.x * dpr, y: selRect.y * dpr, w: selRect.w * dpr, h: selRect.h * dpr };
    }
    if (!dragging) return null;
    const x = Math.min(sx, ex), y = Math.min(sy, ey), w = Math.abs(ex - sx), h = Math.abs(ey - sy);
    if (w < 2 || h < 2) return null;
    return { x: x * dpr, y: y * dpr, w: w * dpr, h: h * dpr };
  }

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    const cw = canvas.width, ch = canvas.height;
    // Dim the whole screen: the window is transparent, so this 45% black sits
    // over the live desktop. The selection punches a transparent hole.
    ctx.clearRect(0, 0, cw, ch);
    ctx.fillStyle = "rgba(0,0,0,0.45)";
    ctx.fillRect(0, 0, cw, ch);

    const r = rectPx();
    if (!r) {
      if (phase === "select") drawHint(ctx, cw, ch);
      return;
    }
    // selection: clear back to full transparency → live desktop at full brightness
    ctx.clearRect(r.x, r.y, r.w, r.h);
    ctx.strokeStyle = "rgba(255,255,255,0.92)";
    ctx.lineWidth = 1.5 * dpr;
    ctx.strokeRect(r.x, r.y, r.w, r.h);
    // annotations, clipped to the selection
    if (phase === "ask" && (shapes.length || cur)) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(r.x, r.y, r.w, r.h);
      ctx.clip();
      paintShapes(ctx, cur ? [...shapes, cur] : shapes, 0, 0);
      ctx.restore();
    }
    // size label while dragging
    if (phase === "select") {
      const px = Math.round(r.w), py = Math.round(r.h);
      const label = `${px} × ${py}`;
      ctx.font = `${12 * dpr}px 'Segoe UI', sans-serif`;
      const tm = ctx.measureText(label);
      const ly = r.y > 26 * dpr ? r.y - 8 * dpr : r.y + r.h + 18 * dpr;
      ctx.fillStyle = "rgba(0,0,0,0.7)";
      ctx.fillRect(r.x, ly - 14 * dpr, tm.width + 12 * dpr, 20 * dpr);
      ctx.fillStyle = "#e7eaf1";
      ctx.fillText(label, r.x + 6 * dpr, ly);
    }
  }

  function drawHint(ctx: CanvasRenderingContext2D, cw: number, ch: number) {
    const label = "拖动框选要问的区域  ·  Esc / 右键取消";
    ctx.font = `${13 * dpr}px 'Segoe UI', sans-serif`;
    const tm = ctx.measureText(label);
    const padX = 16 * dpr, padY = 10 * dpr;
    const bw = tm.width + padX * 2, bh = 18 * dpr + padY * 2;
    const bx = (cw - bw) / 2, by = (ch - bh) / 2;
    ctx.fillStyle = "rgba(24,26,33,0.82)";
    ctx.fillRect(bx, by, bw, bh);
    ctx.strokeStyle = "rgba(255,255,255,0.12)";
    ctx.lineWidth = 1 * dpr;
    ctx.strokeRect(bx, by, bw, bh);
    ctx.fillStyle = "#e7eaf1";
    ctx.textBaseline = "middle";
    ctx.fillText(label, bx + padX, by + bh / 2);
    ctx.textBaseline = "alphabetic";
  }

  // --- annotation drawing ---
  function drawArrow(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number) {
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();
    const ang = Math.atan2(y2 - y1, x2 - x1);
    const len = 13 * dpr;
    ctx.beginPath();
    ctx.moveTo(x2, y2);
    ctx.lineTo(x2 - len * Math.cos(ang - Math.PI / 6), y2 - len * Math.sin(ang - Math.PI / 6));
    ctx.moveTo(x2, y2);
    ctx.lineTo(x2 - len * Math.cos(ang + Math.PI / 6), y2 - len * Math.sin(ang + Math.PI / 6));
    ctx.stroke();
  }

  // Paint shapes onto ctx. Coords are CSS px → ×dpr to physical, minus (ox,oy)
  // origin (0 for the overlay canvas, the crop origin when exporting).
  function paintShapes(ctx: CanvasRenderingContext2D, list: Shape[], ox: number, oy: number) {
    for (const s of list) {
      ctx.strokeStyle = s.color;
      ctx.lineWidth = 3 * dpr;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      if (s.tool === "pen") {
        ctx.beginPath();
        s.points.forEach((p, i) =>
          i ? ctx.lineTo(p.x * dpr - ox, p.y * dpr - oy) : ctx.moveTo(p.x * dpr - ox, p.y * dpr - oy),
        );
        ctx.stroke();
      } else if (s.tool === "rect") {
        const x = Math.min(s.x1, s.x2) * dpr - ox, y = Math.min(s.y1, s.y2) * dpr - oy;
        ctx.strokeRect(x, y, Math.abs(s.x2 - s.x1) * dpr, Math.abs(s.y2 - s.y1) * dpr);
      } else {
        drawArrow(ctx, s.x1 * dpr - ox, s.y1 * dpr - oy, s.x2 * dpr - ox, s.y2 * dpr - oy);
      }
    }
  }

  function undo() {
    shapes = shapes.slice(0, -1);
    draw();
  }

  function inSel(x: number, y: number): boolean {
    return !!selRect && x >= selRect.x && x <= selRect.x + selRect.w && y >= selRect.y && y <= selRect.y + selRect.h;
  }
  function clampSel(x: number, y: number) {
    if (!selRect) return { x, y };
    return {
      x: Math.max(selRect.x, Math.min(selRect.x + selRect.w, x)),
      y: Math.max(selRect.y, Math.min(selRect.y + selRect.h, y)),
    };
  }

  // Bake annotations onto the full-res clean crop (not the downscaled preview)
  // for the vision request, so annotated sends stay crisp.
  async function composite(): Promise<string | undefined> {
    if (!imageBase64 || !selRect) return undefined;
    const base = await new Promise<HTMLImageElement>((res, rej) => {
      const im = new Image();
      im.onload = () => res(im);
      im.onerror = rej;
      im.src = `data:image/png;base64,${imageBase64}`;
    }).catch(() => null);
    if (!base) return undefined;
    const off = document.createElement("canvas");
    off.width = base.naturalWidth;
    off.height = base.naturalHeight;
    const octx = off.getContext("2d");
    if (!octx) return undefined;
    octx.drawImage(base, 0, 0);
    paintShapes(octx, shapes, region.x, region.y);
    return off.toDataURL("image/png").split(",")[1];
  }

  function onDown(e: MouseEvent) {
    if (e.button === 2) { getCurrentWindow().hide(); return; } // 右键：任何阶段都退出（隐藏复用）
    if (e.button !== 0) return;
    if (phase === "ask") {
      // annotation: only when a tool is picked and the press lands in the selection
      if (tool === "none" || !inSel(e.clientX, e.clientY)) return;
      drawingAnno = true;
      cur =
        tool === "pen"
          ? { tool: "pen", color, points: [{ x: e.clientX, y: e.clientY }] }
          : { tool, color, x1: e.clientX, y1: e.clientY, x2: e.clientX, y2: e.clientY };
      return;
    }
    sx = e.clientX; sy = e.clientY; ex = e.clientX; ey = e.clientY;
    dragging = true;
  }
  function onMove(e: MouseEvent) {
    if (drawingAnno && cur) {
      const p = clampSel(e.clientX, e.clientY);
      if (cur.tool === "pen") cur.points.push(p);
      else { cur.x2 = p.x; cur.y2 = p.y; }
      draw();
      return;
    }
    if (!dragging) return;
    ex = e.clientX; ey = e.clientY;
    draw();
  }
  async function onUp(e: MouseEvent) {
    if (e.button !== 0) return;
    if (drawingAnno) {
      drawingAnno = false;
      if (cur) {
        const big = cur.tool === "pen"
          ? cur.points.length > 1
          : Math.abs(cur.x2 - cur.x1) > 3 || Math.abs(cur.y2 - cur.y1) > 3;
        if (big) shapes = [...shapes, cur];
      }
      cur = null;
      draw();
      return;
    }
    if (!dragging) return;
    dragging = false;
    const x = Math.min(sx, ex), y = Math.min(sy, ey), w = Math.abs(ex - sx), h = Math.abs(ey - sy);
    if (w < 8 || h < 8) { draw(); return; }
    selRect = { x, y, w, h };
    region = { x: Math.round(x * dpr), y: Math.round(y * dpr), w: Math.round(w * dpr), h: Math.round(h * dpr) };
    phase = "ask";
    draw();
    await tick();
    inputEl?.focus();
    // Run crop + (optional) OCR as a tracked promise so an early submit can wait
    // for the image instead of sending the first question text-only.
    capturePromise = (async () => {
      try {
        const r = await invoke<{ path: string; base64: string }>("capture_region", {
          x: region.x, y: region.y, width: region.w, height: region.h,
        });
        imageBase64 = r.base64;
        if (config && !config.api.supports_vision) {
          try {
            ocrText = await invoke<string>("ocr_image", { path: r.path, language: config.ocr.language });
          } catch {}
        }
      } catch (e: any) {
        captureError = `截图失败：${e?.message ?? e}`;
      }
    })();
  }

  async function send(t: string) {
    if (!apiConfig) return;
    if (getMessages().length === 0) {
      pendingSend = true;
      try {
        // wait for the crop/OCR so the first turn always carries the image
        if (capturePromise) await capturePromise;
        // Vision + annotations → send the baked composite; otherwise the clean crop.
        let img = imageBase64;
        if (config?.api.supports_vision && shapes.length) {
          const baked = await composite();
          if (baked) img = baked;
        }
        sendMessage(apiConfig, t, img, ocrText);
      } finally {
        pendingSend = false;
      }
    } else {
      sendMessage(apiConfig, t);
    }
  }

  function submit() {
    if (isStreaming()) { cancelStream(); return; }
    const t = text.trim();
    if (!t) return;
    text = "";
    send(t);
  }

  function sendPreset(t: string) {
    if (pendingSend || isStreaming()) return;
    inputEl?.focus();
    send(t);
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
  }

  function pickModel(m: string) {
    menuOpen = false;
    curModel = m;
    if (apiConfig) apiConfig.model = m;
    if (config) {
      config.api.model = m;
      saveConfig(config).catch(() => {});
    }
    inputEl?.focus();
  }

  async function pullModelsLive() {
    if (!apiConfig) return;
    pullingModels = true;
    pullError = null;
    try {
      const list = await fetchModels(apiConfig);
      modelList = list;
      if (!list.length) pullError = "没拉到模型";
      else if (config) {
        config.api.models = list;
        saveConfig(config).catch(() => {});
      }
    } catch (e: any) {
      pullError = `拉取失败：${e?.message || e}`;
    } finally {
      pullingModels = false;
    }
  }

  // Close the model menu on outside click. No full-screen backdrop — that
  // would swallow the canvas right-click-to-hide.
  $effect(() => {
    if (!menuOpen) return;
    const close = (e: MouseEvent) => {
      if (!(e.target as Element)?.closest?.(".model")) menuOpen = false;
    };
    window.addEventListener("mousedown", close, true);
    return () => window.removeEventListener("mousedown", close, true);
  });

  // auto-scroll answers
  $effect(() => {
    if (getMessages().length && msgsEl) {
      requestAnimationFrame(() => { if (msgsEl) msgsEl.scrollTop = msgsEl.scrollHeight; });
    }
  });

  // ask stack position (CSS px), anchored at selection, clamped on-screen
  const STACK_W = 560;
  let stackStyle = $derived.by(() => {
    if (!selRect) return "display:none";
    const m = 12, vw = window.innerWidth, vh = window.innerHeight;
    let left = selRect.x;
    if (left + STACK_W > vw - m) left = vw - STACK_W - m;
    if (left < m) left = m;
    const below = vh - (selRect.y + selRect.h) - 8;
    const above = selRect.y - 8;
    let css = `left:${left}px; width:${STACK_W}px;`;
    if (below >= 160 || below >= above) {
      css += `top:${selRect.y + selRect.h + 8}px; max-height:${Math.max(140, below - m)}px;`;
    } else {
      css += `bottom:${vh - selRect.y + 8}px; max-height:${Math.max(140, above - m)}px;`;
    }
    return css;
  });
</script>

<div class="root">
  <canvas
    bind:this={canvas}
    onmousedown={onDown}
    onmousemove={onMove}
    onmouseup={onUp}
    oncontextmenu={(e) => e.preventDefault()}
    class:asking={phase === "ask"}
    class:drawmode={phase === "ask" && tool !== "none"}
  ></canvas>

  {#if phase === "ask"}
    <div class="stack" style={stackStyle}>
      <div class="tools glass">
        <button class="tl" class:on={tool === "arrow"} title="箭头"
          onclick={() => (tool = tool === "arrow" ? "none" : "arrow")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="7" y1="17" x2="17" y2="7"/><polyline points="9 7 17 7 17 15"/>
          </svg>
        </button>
        <button class="tl" class:on={tool === "rect"} title="方框"
          onclick={() => (tool = tool === "rect" ? "none" : "rect")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="6" width="16" height="12" rx="1.5"/></svg>
        </button>
        <button class="tl" class:on={tool === "pen"} title="画笔"
          onclick={() => (tool = tool === "pen" ? "none" : "pen")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 19l7-7a2.1 2.1 0 0 0-3-3l-7 7-1 4z"/>
          </svg>
        </button>
        <span class="sep"></span>
        {#each COLORS as c}
          <button class="sw" class:on={color === c} style="--c:{c}" title={c}
            onclick={() => (color = c)} aria-label={c}></button>
        {/each}
        <span class="sep"></span>
        <button class="tl undo" title="撤销" onclick={undo} disabled={!shapes.length}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 14L4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 0 10h-1"/>
          </svg>
        </button>
      </div>

      {#if getMessages().length || getError() || captureError}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
        <div class="msgs" bind:this={msgsEl} onclick={onMsgsClick}>
          {#each getMessages() as m, i}
            {#if m.role === "user"}
              <div class="q">问：<b>{m.content}</b></div>
            {:else}
              <div class="a">
                {#if m.content && !(isStreaming() && i === getMessages().length - 1)}
                  <button class="acopy" title="复制回答" onclick={() => copyAnswer(i, m.content)}>
                    {@html copiedIdx === i ? CHECK_SVG : COPY_SVG}
                  </button>
                {/if}
                {#if m.content}{@html renderMd(m.content)}{:else}<span class="dots"><i></i><i></i><i></i></span>{/if}
              </div>
            {/if}
          {/each}
          {#if captureError}<div class="err">{captureError}</div>{/if}
          {#if getError()}<div class="err">{getError()}</div>{/if}
          {#if getMessages().some((m) => m.role === "assistant")}
            <div class="aihint">AI 生成，请自行甄别</div>
          {/if}
        </div>
      {/if}

      {#if !getMessages().length && !pendingSend && !captureError}
        <div class="chips">
          {#each PRESETS as p}
            <button class="chip glass" onclick={() => sendPreset(p.text)}>{p.label}</button>
          {/each}
        </div>
      {/if}

      <div class="bar glass">
        <div class="model">
          <button class="modelpill" title="切换模型" onclick={() => { menuOpen = !menuOpen; pullError = null; }}>
            {curModel || "选择模型"}
          </button>
          {#if menuOpen}
            <div class="menu glass">
              {#if modelList.length}
                {#each modelList as m}
                  <button class="mi" class:on={m === curModel} onclick={() => pickModel(m)}>{m}</button>
                {/each}
              {:else}
                <button class="mi" onclick={pullModelsLive} disabled={pullingModels}>
                  {pullingModels ? "拉取中…" : "拉取模型列表"}
                </button>
              {/if}
              {#if pullError}<div class="mierr">{pullError}</div>{/if}
            </div>
          {/if}
        </div>
        <input
          bind:this={inputEl}
          bind:value={text}
          onkeydown={onInputKey}
          placeholder={getMessages().length ? "继续追问…" : "问点什么…（选区已作为附图）"}
        />
        <button class="send" onclick={submit} title={isStreaming() ? "停止" : "发送"}>
          {#if isStreaming()}
            <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
              <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
            </svg>
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html, body) { background: transparent; }
  .root { position: fixed; inset: 0; overflow: hidden; }
  canvas { position: fixed; inset: 0; width: 100vw; height: 100vh; cursor: crosshair; display: block; }
  canvas.asking { cursor: default; }
  canvas.drawmode { cursor: crosshair; }

  .tools { display: flex; align-items: center; gap: 4px; padding: 6px 8px; }
  .tl {
    width: 30px; height: 30px; flex-shrink: 0; border: 0; border-radius: 8px;
    background: transparent; color: #cdd2dc; cursor: pointer;
    display: flex; align-items: center; justify-content: center; transition: background .12s, color .12s;
  }
  .tl:hover:not(:disabled) { background: rgba(255,255,255,0.08); color: #fff; }
  .tl.on { background: #3a82f6; color: #fff; }
  .tl svg { width: 17px; height: 17px; }
  .tl:disabled { opacity: 0.35; cursor: not-allowed; }
  .tools .sep { width: 1px; height: 18px; background: rgba(255,255,255,0.12); margin: 0 4px; }
  .sw {
    width: 17px; height: 17px; flex-shrink: 0; border-radius: 50%; cursor: pointer;
    background: var(--c); border: 2px solid transparent; box-shadow: 0 0 0 1px rgba(0,0,0,0.3);
  }
  .sw.on { border-color: #fff; }

  .stack {
    position: fixed;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-family: "Segoe UI", "PingFang SC", system-ui, sans-serif;
  }

  .glass {
    background: rgba(24, 26, 33, 0.72);
    backdrop-filter: blur(22px) saturate(1.3);
    -webkit-backdrop-filter: blur(22px) saturate(1.3);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 13px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.45);
  }

  .msgs {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-radius: 13px;
    background: rgba(24, 26, 33, 0.72);
    backdrop-filter: blur(22px) saturate(1.3);
    -webkit-backdrop-filter: blur(22px) saturate(1.3);
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.45);
  }
  .q { font-size: 12px; color: #8b90a0; }
  .q b { color: #cfd4df; font-weight: 600; }
  .a { position: relative; font-size: 13.5px; line-height: 1.62; color: #e7eaf1; }
  .acopy {
    position: absolute; top: 0; right: 0; width: 22px; height: 22px; border: 0; border-radius: 6px;
    background: rgba(255,255,255,0.07); color: #9aa0ad; cursor: pointer; opacity: 0; transition: opacity .12s;
    display: flex; align-items: center; justify-content: center;
  }
  .a:hover .acopy { opacity: 1; }
  .acopy:hover { background: rgba(255,255,255,0.14); color: #fff; }
  .acopy :global(svg), .a :global(.cbcopy svg) { width: 13px; height: 13px; }
  .a :global(.cb) { position: relative; }
  .a :global(.cbcopy) {
    position: absolute; top: 15px; right: 7px; width: 24px; height: 24px; border: 0; border-radius: 6px;
    background: rgba(255,255,255,0.08); color: #9aa0ad; cursor: pointer; opacity: 0; transition: opacity .12s;
    display: flex; align-items: center; justify-content: center;
  }
  .a :global(.cb:hover .cbcopy) { opacity: 1; }
  .a :global(.cbcopy:hover) { background: rgba(255,255,255,0.16); color: #fff; }
  .a :global(code) { font-family: "Cascadia Code", ui-monospace, monospace; font-size: 12.5px; background: rgba(255,255,255,0.08); padding: 1px 6px; border-radius: 5px; }
  .a :global(pre) { margin: 9px 0 2px; padding: 11px 13px; border-radius: 9px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.07); overflow: auto; }
  .a :global(pre code) { background: transparent; padding: 0; font-size: 12.5px; line-height: 1.55; color: #dfe4ee; }
  .a :global(p) { margin: 4px 0; }
  .a :global(ul), .a :global(ol) { padding-left: 18px; margin: 4px 0; }
  .err { font-size: 12px; color: #ff9b9b; }
  .aihint { font-size: 10.5px; color: #6b7280; }

  .dots { display: inline-flex; gap: 4px; }
  .dots i { width: 5px; height: 5px; border-radius: 50%; background: #8b90a0; animation: b 1.2s infinite; }
  .dots i:nth-child(2) { animation-delay: .15s; }
  .dots i:nth-child(3) { animation-delay: .3s; }
  @keyframes b { 0%,60%,100%{opacity:.35;transform:translateY(0)} 30%{opacity:1;transform:translateY(-3px)} }

  .chips { display: flex; gap: 6px; }
  .chip {
    padding: 5px 12px; font-size: 12px; color: #aeb4c0; border: 1px solid rgba(255,255,255,0.09);
    cursor: pointer; border-radius: 9px; transition: background .12s, color .12s;
  }
  .chip:hover { background: rgba(58,130,246,0.22); color: #fff; }

  .model { position: relative; flex-shrink: 0; }
  .modelpill {
    max-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    border: 0; border-radius: 7px; padding: 5px 9px; font-size: 11.5px; color: #8b90a0;
    background: rgba(255,255,255,0.06); cursor: pointer; transition: background .12s, color .12s;
  }
  .modelpill:hover { background: rgba(255,255,255,0.12); color: #cdd2dc; }
  .menu {
    position: absolute; bottom: calc(100% + 10px); left: 0; min-width: 200px; max-width: 300px;
    max-height: 240px; overflow-y: auto; padding: 4px; display: flex; flex-direction: column; gap: 1px;
  }
  .mi {
    border: 0; background: transparent; text-align: left; font-size: 12px; padding: 6px 10px;
    border-radius: 7px; color: #cdd2dc; cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .mi:hover:not(:disabled) { background: rgba(255,255,255,0.08); color: #fff; }
  .mi.on { color: #3a82f6; }
  .mi:disabled { opacity: 0.5; cursor: default; }
  .mierr { font-size: 11px; color: #ff9b9b; padding: 5px 10px; }

  .bar { display: flex; align-items: center; gap: 10px; padding: 11px 12px 11px 12px; }
  .bar input { flex: 1; border: 0; background: transparent; outline: none; font-size: 14px; color: #eef1f6; }
  .bar input::placeholder { color: #8b90a0; }
  .send { width: 32px; height: 32px; flex-shrink: 0; border: 0; border-radius: 9px; cursor: pointer; background: #3a82f6; display: flex; align-items: center; justify-content: center; }
  .send:hover { background: #2f74e6; }
  .send svg { width: 15px; height: 15px; color: #fff; }
</style>
