<div align="center">

<img src="src-tauri/icons/128x128.png" width="88" alt="AI Lens" />

# AI Lens

**框一块屏幕，原地问 AI。**

[**中文**](#中文) · [**English**](#english)

</div>

---

<a id="中文"></a>

## 中文  ·  [Switch to English →](#english)

按下快捷键，屏幕压暗。框住你好奇的那块，打一句话——答案就在屏幕上原地流出来。想追问就接着问，按 Esc 回去干活。安装包 3 MB。

### 它做什么

- 覆盖层是透明的，直接盖在实时桌面上——按下那一刻就在框选了，中间没有编码、没有等待。
- 问题发出去的瞬间屏幕就还给你：回答在选区旁的小浮窗里慢慢流，你该干嘛干嘛，想到了再回来追问。
- 在选区上画个箭头、圈个框、涂两笔再问，AI 看到的就是你画完的样子。
- OpenAI、DeepSeek、Ollama、OpenRouter、Azure、Anthropic、Gemini 都能接。设置里切换，模型列表一键拉取。
- 纯文本模型也能用：截图先过本地 Windows OCR，变成文字再发过去。
- 一个小色环跟着鼠标走，演示、录屏时观众一眼能跟上你的光标。半径、颜色、透明度都在设置里，截图永远拍不到它。
- 升级不用跑 GitHub：托盘菜单会自己冒出「更新到 vX.Y.Z」，点一下，下载、验签、安装一气呵成。
- 跑在 Windows 10/11 自带的 WebView2 上，装完约 11 MB。

### 安装

从 [Releases](../../releases) 下载 `AI Lens_x.y.z_x64-setup.exe`，运行即可。

### 用法

1. **Ctrl + Shift + S** —— 屏幕压暗，拖动框选。想换键去设置里改。
2. 工具栏里挑个工具在选区上画，或者直接打字。
3. **回车**发送，答案原地流式展开，可以接着追问。
4. **Esc** 或右键退出。

设置在托盘图标里：服务商、API Key、模型——点**拉取**直接从列表里挑。

### 未来计划

- [ ] 答案、代码块一键复制
- [ ] 输入条上的快捷指令——解释 / 翻译 / 总结选区
- [ ] 剪贴板里的图、拖进来的图也能直接问
- [ ] 托盘里翻看历史会话
- [ ] 标注工具栏加文字、高亮、马赛克
- [ ] OCR 纯文字模式：框一下，文字到手
- [ ] 框选外文，译文原地盖上去
- [ ] 钉图：截完钉在屏幕上
- [ ] 问答时直接换模型
- [ ] 答案朗读

### 从源码构建

```bash
pnpm install
pnpm tauri dev      # 开发运行
pnpm tauri build    # 打安装包
```

需要 Rust 和 Node（pnpm）。Tauri v2 + Svelte 5；截屏用 xcap，OCR 走 WinRT，流式经 Tauri HTTP 插件。

### 协议

[GPL-3.0](LICENSE)

---

<a id="english"></a>

## English  ·  [切换到中文 →](#中文)

Press the hotkey and the screen dims. Drag over the thing you're curious about, type a question — the answer streams out right there, on top of your screen. Follow up as many times as you want, hit Esc, you're back to work. The installer is 3 MB.

### What it does

- The overlay is a transparent layer over your live desktop, so the capture has nothing to encode and nothing to wait for — press, and you're already framing.
- The moment you hit send, the screen is yours again: the answer streams into a small floating panel beside your selection while you keep working, and follow-ups happen right there.
- Draw an arrow at the part you mean, box it, scribble on it. The AI sees exactly what you drew.
- Talks to OpenAI, DeepSeek, Ollama, OpenRouter, Azure, Anthropic, and Gemini. Switch in Settings; one click pulls the provider's model list.
- Text-only model? The screenshot runs through Windows OCR first and arrives as text.
- A small colored ring rides along with your cursor, so viewers can follow your pointer in demos and recordings. Radius, color and opacity sit in Settings — and it never appears in your screenshots.
- Updates come to you: the tray menu flags a new version, and one click downloads, verifies and installs it.
- Runs on the WebView2 already inside Windows 10/11 — install lands at ~11 MB.

### Install

Grab `AI Lens_x.y.z_x64-setup.exe` from [Releases](../../releases) and run it.

### Usage

1. **Ctrl + Shift + S** — screen dims, drag to select. The hotkey lives in Settings if you want a different one.
2. Pick a tool from the toolbar and mark up the selection, or skip straight to typing.
3. **Enter** sends. The answer streams in place; keep asking.
4. **Esc** or right-click leaves.

Settings sit in the tray icon: provider, API key, model — hit **Pull** and pick from the list.

### Roadmap

- [ ] Copy an answer or a code block with one click
- [ ] Quick prompts on the input bar — explain / translate / summarize the selection
- [ ] Ask about an image from the clipboard or a dropped file
- [ ] Browse past conversations from the tray
- [ ] Text labels, highlight and mosaic in the annotation toolbar
- [ ] OCR-only mode: select, grab the text, done
- [ ] Translate the selection and overlay the result in place
- [ ] Pin a screenshot on top of the screen
- [ ] Switch models right from the ask bar
- [ ] Read answers aloud

### Build from source

```bash
pnpm install
pnpm tauri dev      # run in dev
pnpm tauri build    # package an installer
```

Rust + Node with pnpm. Tauri v2, Svelte 5; capture by xcap, OCR via WinRT, streaming through the Tauri HTTP plugin.

### License

[GPL-3.0](LICENSE)
