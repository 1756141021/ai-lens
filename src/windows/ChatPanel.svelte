<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Marked } from "marked";
  import DOMPurify from "dompurify";
  import hljs from "highlight.js";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { loadConfig, saveConfig, toApiConfig, toApiWeb, type AppConfig } from "../lib/config";
  import { fetchModels, type ApiConfig } from "../lib/api";
  import {
    getMessages,
    isStreaming,
    getError,
    getToolStatus,
    sendMessage,
    cancelStream,
  } from "../lib/chat.svelte";

  type Seed = { text: string; image?: string | null; ocr?: string | null };

  let config: AppConfig | null = null;
  let apiConfig: ApiConfig | null = null;

  let text = $state("");
  let inputEl = $state<HTMLTextAreaElement>();
  let msgsEl = $state<HTMLDivElement>();
  let copiedIdx = $state(-1);
  let staged = $state<{ img: string; ocr?: string } | null>(null);
  let lightbox = $state<string | null>(null);

  let curModel = $state("");
  let modelList = $state<string[]>([]);
  let menuOpen = $state(false);
  let pullingModels = $state(false);
  let pullError = $state<string | null>(null);
  let webOn = $state(false);

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
  // Model output is untrusted (prompt injection) — sanitize before {@html}.
  const renderMd = (c: string) => DOMPurify.sanitize(marked.parse(c) as string);

  // This window opened with one captured screenshot — show it in the strip.
  const firstImg = $derived.by(() => {
    const ms = getMessages();
    for (const m of ms) if (m.role === "user" && m.imageBase64) return m.imageBase64;
    return undefined;
  });
  const lastQ = $derived.by(() => {
    const ms = getMessages();
    for (let i = ms.length - 1; i >= 0; i--) if (ms[i].role === "user") return ms[i].content;
    return "";
  });

  async function copyText(t: string): Promise<boolean> {
    try { await writeText(t); return true; } catch { return false; }
  }

  async function onMsgsClick(e: MouseEvent) {
    const btn = (e.target as Element).closest?.(".cbcopy") as HTMLButtonElement | null;
    if (!btn) return;
    const code = btn.parentElement?.querySelector("pre code");
    const ok = await copyText(code?.textContent ?? "");
    btn.innerHTML = ok ? CHECK_SVG : FAIL_SVG;
    setTimeout(() => { if (btn.isConnected) btn.innerHTML = COPY_SVG; }, 1200);
  }

  async function copyAnswer(i: number, content: string) {
    if (!(await copyText(content))) return;
    copiedIdx = i;
    setTimeout(() => { if (copiedIdx === i) copiedIdx = -1; }, 1200);
  }

  function pickModel(m: string) {
    menuOpen = false;
    curModel = m;
    if (apiConfig) apiConfig.model = m;
    if (config) { config.api.model = m; saveConfig(config).catch(() => {}); }
    inputEl?.focus();
  }

  // Global switch, same in-place persistence as the model pill; the way
  // (服务商自带 / 应用内) is chosen in Settings.
  function toggleWeb() {
    if (!config) return;
    webOn = !webOn;
    config.web = { ...config.web, enabled: webOn };
    if (apiConfig) apiConfig.web = toApiWeb(config.web);
    saveConfig(config).catch(() => {});
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
      else if (config) { config.api.models = list; saveConfig(config).catch(() => {}); }
    } catch (e: any) {
      pullError = `拉取失败：${e?.message || e}`;
    } finally {
      pullingModels = false;
    }
  }

  $effect(() => {
    if (!menuOpen) return;
    const close = (e: MouseEvent) => {
      if (!(e.target as Element)?.closest?.(".model")) menuOpen = false;
    };
    window.addEventListener("mousedown", close, true);
    return () => window.removeEventListener("mousedown", close, true);
  });

  $effect(() => {
    if (getMessages().length && msgsEl) {
      requestAnimationFrame(() => { if (msgsEl) msgsEl.scrollTop = msgsEl.scrollHeight; });
    }
  });

  function autosize() {
    if (!inputEl) return;
    inputEl.style.height = "auto";
    inputEl.style.height = Math.min(inputEl.scrollHeight, 96) + "px";
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
  }

  function submit() {
    if (isStreaming()) { cancelStream(); return; }
    const t = text.trim();
    if (!t) return;
    text = "";
    if (inputEl) inputEl.style.height = "auto";
    if (!apiConfig) return;
    const s = staged;
    staged = null;
    sendMessage(apiConfig, t, s?.img, s?.ocr);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (lightbox) { lightbox = null; return; }
      if (menuOpen) { menuOpen = false; inputEl?.focus(); return; }
      getCurrentWindow().close();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    const unlisten = listen<{ image: string; ocr?: string | null }>("stage-image", (e) => {
      staged = { img: e.payload.image, ocr: e.payload.ocr ?? undefined };
      inputEl?.focus();
    });
    // Keep the 🌐 pill honest when the switch is flipped in Settings or
    // another window. Model choice stays per-panel on purpose.
    const unlistenCfg = listen<AppConfig>("config-changed", (e) => {
      if (config) config.web = e.payload.web;
      webOn = !!e.payload.web?.enabled;
      if (apiConfig) apiConfig.web = toApiWeb(e.payload.web);
    });

    (async () => {
      try {
        config = await loadConfig();
        apiConfig = toApiConfig(config);
        curModel = config.api.model;
        modelList = config.api.models ?? [];
        webOn = !!config.web?.enabled;
      } catch {}

      let seed: Seed;
      try {
        seed = await invoke<Seed>("get_chat_seed");
      } catch {
        getCurrentWindow().close();
        return;
      }

      // Fire the first turn (image + question) — its answer streams here.
      if (apiConfig) {
        sendMessage(apiConfig, seed.text, seed.image ?? undefined, seed.ocr ?? undefined);
      }

      const w = getCurrentWindow();
      await w.show();
      await w.setFocus();
      await tick();
      inputEl?.focus();
    })();

    return () => {
      window.removeEventListener("keydown", onKey);
      unlisten.then((u) => u());
      unlistenCfg.then((u) => u());
    };
  });
</script>

<div class="panel">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="strip" data-tauri-drag-region oncontextmenu={(e) => e.preventDefault()}>
    {#if firstImg}
      <button class="curimg" title="查看截图" aria-label="查看截图" onclick={() => (lightbox = firstImg ?? null)}>
        <img src="data:image/png;base64,{firstImg}" alt="" />
      </button>
    {/if}
    <span class="curq">{lastQ}</span>
    <button class="x" title="关闭 (Esc)" onclick={() => getCurrentWindow().close()} aria-label="关闭">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round">
        <line x1="6" y1="6" x2="18" y2="18"/><line x1="18" y1="6" x2="6" y2="18"/>
      </svg>
    </button>
  </div>

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
    {#if getToolStatus()}<div class="toolstatus">{getToolStatus()}</div>{/if}
    {#if getError()}<div class="err">{getError()}</div>{/if}
    {#if getMessages().some((m) => m.role === "assistant")}
      <div class="aihint">AI 生成，请自行甄别</div>
    {/if}
  </div>

  {#if staged}
    <div class="staged">
      <img src="data:image/png;base64,{staged.img}" alt="" />
      <span>已附截图，随下一条问题发出</span>
      <button class="rm" title="移除" aria-label="移除" onclick={() => (staged = null)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round">
          <line x1="6" y1="6" x2="18" y2="18"/><line x1="18" y1="6" x2="6" y2="18"/>
        </svg>
      </button>
    </div>
  {/if}

  <div class="bar">
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
    <button class="webpill" class:on={webOn} title={webOn ? "联网搜索：开" : "联网搜索：关"} aria-label="联网搜索" onclick={toggleWeb}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
        <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/>
        <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
      </svg>
    </button>
    <textarea
      rows="1"
      bind:this={inputEl}
      bind:value={text}
      onkeydown={onInputKey}
      oninput={autosize}
      placeholder="继续追问…"
    ></textarea>
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

  {#if lightbox}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <button class="lightbox" onclick={() => (lightbox = null)} aria-label="关闭预览">
      <img src="data:image/png;base64,{lightbox}" alt="" />
    </button>
  {/if}
</div>

<style>
  :global(html, body) { background: transparent; }

  .panel {
    position: absolute; inset: 0;
    display: flex; flex-direction: column; gap: 8px;
    padding: 8px;
    border-radius: 14px;
    background: rgba(24, 26, 33, 0.82);
    backdrop-filter: blur(22px) saturate(1.3);
    -webkit-backdrop-filter: blur(22px) saturate(1.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-sizing: border-box;
    overflow: hidden;
    font-family: "Segoe UI", "PingFang SC", system-ui, sans-serif;
  }

  .glass {
    background: rgba(24, 26, 33, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 11px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.45);
  }

  .strip { height: 26px; flex-shrink: 0; display: flex; align-items: center; padding: 0 3px 0 2px; }
  .curimg {
    height: 18px; max-width: 36px; padding: 0; margin-left: 4px; flex-shrink: 0;
    border: 0; border-radius: 4px; background: transparent; cursor: pointer;
    outline: 1px solid rgba(255, 255, 255, 0.14); outline-offset: -1px;
    display: flex; overflow: hidden;
  }
  .curimg:hover { outline-color: #3a82f6; }
  .curimg img { height: 18px; max-width: 36px; object-fit: cover; display: block; }
  .curq {
    flex: 1; min-width: 0; padding-left: 8px;
    font-size: 12px; color: #aeb4c0;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    pointer-events: none;
  }
  .strip .x {
    width: 22px; height: 22px; border: 0; border-radius: 6px; background: transparent;
    color: #8b90a0; cursor: pointer; display: flex; align-items: center; justify-content: center;
  }
  .strip .x:hover { background: rgba(255,255,255,0.1); color: #fff; }
  .strip .x svg { width: 11px; height: 11px; }

  .msgs {
    flex: 1 1 auto; min-height: 0;
    overflow-y: auto; display: flex; flex-direction: column; gap: 10px;
    padding: 4px 8px;
  }
  .q { font-size: 12px; color: #8b90a0; white-space: pre-wrap; overflow-wrap: anywhere; }
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
  .a :global(a) {
    color: #6ea8ff; text-decoration: underline;
    text-decoration-color: rgba(110, 168, 255, 0.35); text-underline-offset: 2px; overflow-wrap: anywhere;
  }
  .a :global(a:hover) { color: #9cc3ff; text-decoration-color: currentColor; }
  .a :global(ul), .a :global(ol) { padding-left: 18px; margin: 4px 0; }
  .err { font-size: 12px; color: #ff9b9b; white-space: pre-line; overflow-wrap: anywhere; }
  .aihint { font-size: 10.5px; color: #6b7280; }
  .toolstatus { display: flex; align-items: center; gap: 7px; font-size: 11.5px; color: #7fa3e0; }
  .toolstatus::before {
    content: ""; width: 5px; height: 5px; border-radius: 50%; flex-shrink: 0;
    background: currentColor; animation: b 1.2s infinite;
  }

  .dots { display: inline-flex; gap: 4px; }
  .dots i { width: 5px; height: 5px; border-radius: 50%; background: #8b90a0; animation: b 1.2s infinite; }
  .dots i:nth-child(2) { animation-delay: .15s; }
  .dots i:nth-child(3) { animation-delay: .3s; }
  @keyframes b { 0%,60%,100%{opacity:.35;transform:translateY(0)} 30%{opacity:1;transform:translateY(-3px)} }

  .staged {
    flex-shrink: 0; display: flex; align-items: center; gap: 9px; padding: 6px 8px;
    font-size: 11.5px; color: #aeb4c0; border-radius: 10px; background: rgba(255,255,255,0.05);
  }
  .staged img {
    height: 30px; max-width: 52px; object-fit: cover; border-radius: 5px;
    outline: 1px solid rgba(255,255,255,0.14); outline-offset: -1px;
  }
  .staged span { flex: 1; min-width: 0; }
  .staged .rm {
    width: 22px; height: 22px; flex-shrink: 0; border: 0; border-radius: 6px;
    background: transparent; color: #8b90a0; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
  }
  .staged .rm:hover { background: rgba(255,255,255,0.1); color: #fff; }
  .staged .rm svg { width: 11px; height: 11px; }

  .bar { flex-shrink: 0; display: flex; align-items: center; gap: 10px; padding: 7px 8px; }
  .model { position: relative; flex-shrink: 0; }
  .webpill {
    width: 26px; height: 26px; flex-shrink: 0; border: 0; border-radius: 7px;
    background: rgba(255,255,255,0.06); color: #8b90a0; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    transition: background .12s, color .12s;
  }
  .webpill:hover { background: rgba(255,255,255,0.12); color: #cdd2dc; }
  .webpill.on { color: #5a9bff; background: rgba(58,130,246,0.16); }
  .webpill svg { width: 14px; height: 14px; }
  .modelpill {
    max-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    border: 0; border-radius: 7px; padding: 5px 9px; font-size: 11.5px; color: #8b90a0;
    background: rgba(255,255,255,0.06); cursor: pointer; transition: background .12s, color .12s;
  }
  .modelpill:hover { background: rgba(255,255,255,0.12); color: #cdd2dc; }
  .menu {
    position: absolute; bottom: calc(100% + 10px); left: 0; min-width: 200px; max-width: 300px;
    max-height: min(240px, calc(100vh - 96px)); overflow-y: auto; padding: 4px;
    display: flex; flex-direction: column; gap: 1px;
  }
  .mi {
    border: 0; background: transparent; text-align: left; font-size: 12px; padding: 6px 10px;
    border-radius: 7px; color: #cdd2dc; cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .mi:hover:not(:disabled) { background: rgba(255,255,255,0.08); color: #fff; }
  .mi.on { color: #3a82f6; }
  .mi:disabled { opacity: 0.5; cursor: default; }
  .mierr { font-size: 11px; color: #ff9b9b; padding: 5px 10px; white-space: pre-line; overflow-wrap: anywhere; }

  .bar textarea {
    flex: 1; border: 0; background: transparent; outline: none;
    font-size: 14px; color: #eef1f6; font-family: inherit;
    resize: none; line-height: 1.45; max-height: 96px; overflow-y: auto; padding: 0; margin: 0;
  }
  .bar textarea::placeholder { color: #8b90a0; }
  .send { width: 32px; height: 32px; flex-shrink: 0; border: 0; border-radius: 9px; cursor: pointer; background: #3a82f6; display: flex; align-items: center; justify-content: center; }
  .send:hover { background: #2f74e6; }
  .send svg { width: 15px; height: 15px; color: #fff; }

  .lightbox {
    position: absolute; inset: 0; z-index: 20; border: 0; padding: 16px; margin: 0;
    display: flex; align-items: center; justify-content: center; cursor: zoom-out;
    background: rgba(10,11,15,0.88); border-radius: 14px;
  }
  .lightbox img { max-width: 100%; max-height: 100%; object-fit: contain; border-radius: 6px; }
</style>
