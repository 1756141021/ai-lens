<div align="center">

<img src="src-tauri/icons/128x128.png" width="88" alt="AI Lens" />

# AI Lens

**Screenshot a region → ask AI, right where you are.**

[**English**](#english) · [**中文**](#中文)

</div>

---

<a id="english"></a>

## English  ·  [切换到中文 →](#中文)

AI Lens is a lightweight Windows tool: press a hotkey, select a screen region, annotate it, and ask any AI about it — the answer streams **in place** over your screen. ~3 MB download, no Electron.

### Features
- 📸 **Instant capture** — a transparent overlay over the live desktop (QQ-style), zero encode latency.
- 💬 **Ask in place** — select a region, type, the answer streams right there. Multi-turn.
- ✏️ **Annotate** — arrow / rectangle / pen + colors, baked into what the AI sees.
- 🔌 **Any provider** — OpenAI-compatible (OpenAI / DeepSeek / Ollama / OpenRouter / Azure), Anthropic (native), Google Gemini (native). Switch in Settings.
- 🧲 **Model pull** — fetch the provider's model list with one click.
- 👁️ **Vision or OCR** — send the image to vision models, or fall back to local Windows OCR for text-only models.
- 🪶 **Tiny** — ~3 MB installer, uses the system WebView2 (no bundled browser).

### Install
Download the latest `AI Lens_x.y.z_x64-setup.exe` from [Releases](../../releases) and run it. Windows 10/11 (WebView2 is built in).

### Usage
1. Press **Ctrl + Shift + S** (configurable) — the screen dims; drag to select a region.
2. *(Optional)* pick a tool from the toolbar and draw on the selection.
3. Type your question and press **Enter** — the answer streams in place.
4. **Esc** or **right-click** to cancel.

Open **Settings** from the tray icon to set your provider, API key, and model (click **Pull** to load the model list).

### Build from source
```bash
pnpm install
pnpm tauri dev      # run in dev
pnpm tauri build    # package an installer
```
Requires Rust + Node (pnpm). Built with Tauri v2 and Svelte 5.

### Tech
Tauri v2 · Svelte 5 · Rust (xcap capture, WinRT OCR) · streaming via the Tauri HTTP plugin.

---

<a id="中文"></a>

## 中文  ·  [Switch to English →](#english)

AI Lens 是个轻量的 Windows 工具：按下快捷键，框选屏幕一块区域，画两笔，直接问 AI——答案就在**原地**流式展开。安装包约 3 MB，不是 Electron。

### 功能
- 📸 **即时截图** —— 透明覆盖层盖在实时桌面上（QQ 式），零编码延迟。
- 💬 **就地问答** —— 框选、打字，答案原地流式出，支持多轮。
- ✏️ **标注** —— 箭头 / 方框 / 画笔 + 颜色，烤进发给 AI 的图里。
- 🔌 **任意服务商** —— OpenAI 兼容（OpenAI / DeepSeek / Ollama / OpenRouter / Azure）、Anthropic 原生、Google Gemini 原生，设置里切换。
- 🧲 **模型拉取** —— 一键拉取服务商的模型列表。
- 👁️ **视觉 / OCR** —— 给视觉模型直接发图，纯文本模型走本地 Windows OCR。
- 🪶 **小** —— 安装包约 3 MB，用系统自带 WebView2，不打包浏览器。

### 安装
从 [Releases](../../releases) 下载最新 `AI Lens_x.y.z_x64-setup.exe` 运行即可。Windows 10/11 自带 WebView2。

### 用法
1. 按 **Ctrl + Shift + S**（可改）—— 屏幕压暗，拖动框选区域。
2. *（可选）* 从工具栏选个工具，在选区上画。
3. 打字提问按 **回车** —— 答案原地流式展开。
4. **Esc** 或 **右键** 取消。

托盘图标打开**设置**，填服务商 / API Key / 模型（点**拉取**加载模型列表）。

### 从源码构建
```bash
pnpm install
pnpm tauri dev      # 开发运行
pnpm tauri build    # 打安装包
```
需要 Rust + Node(pnpm)。基于 Tauri v2、Svelte 5。

### 技术
Tauri v2 · Svelte 5 · Rust（xcap 截屏、WinRT OCR）· 流式走 Tauri HTTP 插件。
