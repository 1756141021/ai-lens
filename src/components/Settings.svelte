<script lang="ts">
  import { untrack } from "svelte";
  import { saveConfig, type AppConfig } from "../lib/config";
  import { fetchModels } from "../lib/api";

  interface Props {
    config: AppConfig;
    onclose: () => void;
  }

  let { config, onclose }: Props = $props();

  // One-time snapshot of the incoming config — the form edits a local copy and
  // commits only on Save. untrack() takes the initial values without creating a
  // reactive dependency on the prop.
  const init = untrack(() => ({
    provider: config.api.provider || "openai",
    baseUrl: config.api.base_url,
    apiKey: config.api.api_key,
    model: config.api.model,
    supportsVision: config.api.supports_vision,
    authHeader: config.api.auth_header,
    apiVersion: config.api.api_version,
    models: config.api.models ?? [],
    hotkey: config.hotkey,
    maxCount: config.cache.max_count,
    ocrLanguage: config.ocr.language,
    cursorEnabled: config.cursor?.enabled ?? true,
    cursorRadius: config.cursor?.radius ?? 16,
    cursorOpacity: config.cursor?.opacity ?? 0.4,
    cursorColor: config.cursor?.color ?? "#ff3b30",
  }));

  let provider = $state(init.provider);
  let baseUrl = $state(init.baseUrl);
  let apiKey = $state(init.apiKey);
  let model = $state(init.model);
  let supportsVision = $state(init.supportsVision);
  let authHeader = $state(init.authHeader);
  let apiVersion = $state(init.apiVersion);
  let hotkey = $state(init.hotkey);
  let maxCount = $state(init.maxCount);
  let ocrLanguage = $state(init.ocrLanguage);
  let cursorEnabled = $state(init.cursorEnabled);
  let cursorRadius = $state(init.cursorRadius);
  let cursorOpacity = $state(init.cursorOpacity);
  let cursorColor = $state(init.cursorColor);

  let saving = $state(false);
  let saveError = $state<string | null>(null);

  let models = $state<string[]>(init.models);
  let loadingModels = $state(false);
  let modelsError = $state<string | null>(null);

  async function pullModels() {
    loadingModels = true;
    modelsError = null;
    try {
      const list = await fetchModels({
        provider: provider as any,
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
        supportsVision,
        authHeader: authHeader.trim() || "Authorization",
        apiVersion: apiVersion.trim(),
      });
      models = list;
      if (!list.length) modelsError = "没拉到模型（接口可能不支持列表）";
      else if (!model.trim()) model = list[0];
    } catch (e: any) {
      modelsError = `拉取失败：${e?.message || e}`;
    } finally {
      loadingModels = false;
    }
  }

  // Per-provider hints. Base URL can be left blank → official endpoint is used.
  const presets: Record<string, { urlPh: string; modelPh: string }> = {
    openai: { urlPh: "https://api.openai.com/v1", modelPh: "gpt-4o" },
    anthropic: { urlPh: "留空用官方地址 api.anthropic.com", modelPh: "claude-sonnet-4-6" },
    gemini: { urlPh: "留空用官方地址 generativelanguage.googleapis.com", modelPh: "gemini-2.0-flash" },
  };
  let preset = $derived(presets[provider] ?? presets.openai);

  async function handleSave() {
    saving = true;
    saveError = null;
    const next: AppConfig = {
      api: {
        provider,
        base_url: baseUrl.trim(),
        api_key: apiKey.trim(),
        model: model.trim(),
        supports_vision: supportsVision,
        auth_header: authHeader.trim() || "Authorization",
        api_version: apiVersion.trim() || "2024-10-21",
        models: $state.snapshot(models),
      },
      hotkey: hotkey.trim() || "ctrl+shift+s",
      cache: { max_count: Math.max(1, Math.floor(Number(maxCount) || 20)) },
      ocr: { language: ocrLanguage.trim() || "zh-Hans" },
      cursor: {
        enabled: cursorEnabled,
        radius: Math.max(4, Math.min(64, Math.floor(Number(cursorRadius) || 16))),
        opacity: Math.max(0.05, Math.min(1, Number(cursorOpacity) || 0.4)),
        color: /^#[0-9a-fA-F]{6}$/.test(cursorColor) ? cursorColor : "#ff3b30",
      },
    };
    try {
      await saveConfig(next);
      onclose();
    } catch (e: any) {
      saveError = e?.message || String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="settings">
  <div class="body">
    <section>
      <span class="sec">API</span>
      <label>
        <span class="lb">服务商 (Provider)</span>
        <select bind:value={provider} onchange={() => (models = [])}>
          <option value="openai">OpenAI 兼容（OpenAI / DeepSeek / Ollama / OpenRouter / Azure…）</option>
          <option value="anthropic">Anthropic（Claude）</option>
          <option value="gemini">Google Gemini</option>
        </select>
      </label>
      <label>
        <span class="lb">接口地址 (Base URL)</span>
        <input type="text" bind:value={baseUrl} placeholder={preset.urlPh} spellcheck="false" />
        {#if provider === "openai"}
          <span class="hint">多余的 /chat/completions 会自动处理。Azure 填完整地址即可。</span>
        {:else}
          <span class="hint">留空就用官方地址；走网关/代理才需要填。</span>
        {/if}
      </label>
      <label>
        <span class="lb">API Key</span>
        <input type="password" bind:value={apiKey} placeholder="sk-..." spellcheck="false" />
      </label>
      <label>
        <span class="lb">模型 (Model)</span>
        <div class="modelrow">
          <input type="text" bind:value={model} placeholder={preset.modelPh} spellcheck="false" />
          <button type="button" class="pull" onclick={pullModels} disabled={loadingModels}>
            {loadingModels ? "拉取中…" : "拉取"}
          </button>
        </div>
        {#if models.length}
          <select class="modelpick" value={model} onchange={(e) => (model = e.currentTarget.value)}>
            {#each models as m}<option value={m}>{m}</option>{/each}
          </select>
        {/if}
        {#if modelsError}
          <span class="hint err">{modelsError}</span>
        {:else if models.length}
          <span class="hint">已拉取 {models.length} 个，上面下拉选或直接在框里改</span>
        {/if}
      </label>
      <label class="row">
        <input type="checkbox" bind:checked={supportsVision} />
        <span>视觉识图</span>
      </label>
      <span class="hint">
        开：把截图<b>直接发给 AI 看</b>（需模型支持图片，多数现代模型都支持，推荐开着）。<br />
        关：本地先把截图里的<b>文字识别出来（OCR）</b>，只发文字——给不支持图片的纯文本模型用。
      </span>
    </section>

    <section>
      <span class="sec">光标高亮</span>
      <label class="row">
        <input type="checkbox" bind:checked={cursorEnabled} />
        <span>跟随鼠标显示圆圈</span>
      </label>
      {#if cursorEnabled}
        <label>
          <span class="lb">半径（px）</span>
          <input type="number" min="4" max="64" bind:value={cursorRadius} />
        </label>
        <label>
          <span class="lb">不透明度（{Math.round(cursorOpacity * 100)}%）</span>
          <input type="range" min="0.05" max="1" step="0.05" bind:value={cursorOpacity} />
        </label>
        <label>
          <span class="lb">颜色</span>
          <input type="color" bind:value={cursorColor} />
        </label>
      {/if}
    </section>

    <details class="adv">
      <summary>高级</summary>
      <div class="advbody">
        <label>
          <span class="lb">Auth Header</span>
          <input type="text" bind:value={authHeader} placeholder="Authorization" spellcheck="false" />
        </label>
        <label>
          <span class="lb">API Version</span>
          <input type="text" bind:value={apiVersion} placeholder="2024-10-21" spellcheck="false" />
          <span class="hint">Azure 用作 api-version；Anthropic 用作 anthropic-version（默认 2023-06-01）。其它服务商忽略。</span>
        </label>
        <label>
          <span class="lb">快捷键</span>
          <input type="text" bind:value={hotkey} placeholder="ctrl+shift+s" spellcheck="false" />
        </label>
        <label>
          <span class="lb">缓存上限（张）</span>
          <input type="number" min="1" bind:value={maxCount} />
        </label>
        <label>
          <span class="lb">OCR 语言</span>
          <input type="text" bind:value={ocrLanguage} placeholder="zh-Hans" spellcheck="false" />
          <span class="hint">仅当关闭"视觉识图"时生效：本地识别截图文字用哪种语言。zh-Hans=简体中文，en=英文。</span>
        </label>
      </div>
    </details>

    {#if saveError}
      <div class="error">{saveError}</div>
    {/if}
  </div>

  <footer>
    <button class="cancel" onclick={onclose} disabled={saving}>取消</button>
    <button class="save" onclick={handleSave} disabled={saving}>
      {saving ? "保存中…" : "保存"}
    </button>
  </footer>
</div>

<style>
  .settings {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: #181a21;
    color: #e7eaf1;
    font-family: "Segoe UI", "PingFang SC", system-ui, sans-serif;
  }

  .body {
    flex: 1;
    overflow-y: auto;
    padding: 18px 18px 8px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-height: 0;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 13px;
  }

  .sec {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #6b7280;
    font-weight: 600;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .lb { font-size: 12.5px; color: #aeb4c0; }
  .hint { font-size: 11px; color: #6b7280; line-height: 1.55; }
  .hint b { color: #aeb4c0; font-weight: 600; }
  .hint.err { color: #ff9b9b; }

  .modelrow { display: flex; gap: 8px; }
  .modelrow input { flex: 1; }
  .pull {
    flex-shrink: 0;
    padding: 0 14px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: #23262f;
    color: #cdd2dc;
    font-size: 12.5px;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s;
  }
  .pull:hover:not(:disabled) { background: #2b2f3a; border-color: #3a82f6; color: #eef1f6; }
  .pull:disabled { opacity: 0.5; cursor: not-allowed; }

  label.row {
    flex-direction: row;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    color: #cdd2dc;
  }

  input[type="text"],
  input[type="password"],
  input[type="number"],
  select {
    background: #23262f;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 8px;
    padding: 9px 12px;
    color: #eef1f6;
    font-size: 13.5px;
    font-family: inherit;
    outline: none;
    transition: border-color 0.12s, box-shadow 0.12s;
    user-select: text;
    -webkit-user-select: text;
  }
  input::placeholder { color: #5f6573; }
  input:focus,
  select:focus {
    border-color: #3a82f6;
    box-shadow: 0 0 0 3px rgba(58, 130, 246, 0.18);
  }
  select { cursor: pointer; appearance: none; }
  select option { background: #23262f; color: #eef1f6; }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: #3a82f6;
    cursor: pointer;
  }

  input[type="range"] {
    accent-color: #3a82f6;
    cursor: pointer;
  }

  input[type="color"] {
    width: 44px;
    height: 28px;
    padding: 2px;
    background: #23262f;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 8px;
    cursor: pointer;
  }

  .adv { border-top: 1px solid rgba(255, 255, 255, 0.07); padding-top: 14px; }
  .adv summary {
    font-size: 12.5px;
    color: #9aa0ad;
    cursor: pointer;
    list-style: none;
    user-select: none;
  }
  .adv summary::before { content: "▸ "; color: #6b7280; }
  .adv[open] summary::before { content: "▾ "; }
  .advbody { display: flex; flex-direction: column; gap: 13px; padding-top: 13px; }

  .error {
    color: #ff9b9b;
    font-size: 12px;
    padding: 9px 11px;
    background: rgba(255, 80, 80, 0.08);
    border: 1px solid rgba(255, 80, 80, 0.18);
    border-radius: 8px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 9px;
    padding: 12px 18px;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
  }

  footer button {
    padding: 8px 18px;
    border-radius: 8px;
    font-size: 13px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.12s;
  }

  .cancel {
    background: transparent;
    color: #aeb4c0;
    border-color: rgba(255, 255, 255, 0.12);
  }
  .cancel:hover:not(:disabled) { background: rgba(255, 255, 255, 0.06); color: #eef1f6; }

  .save {
    background: #3a82f6;
    color: #fff;
  }
  .save:hover:not(:disabled) { background: #2f74e6; }

  footer button:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
