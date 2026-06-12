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
- OpenAI、DeepSeek、Ollama、OpenRouter、Azure、Anthropic、Gemini 都能接。设置里切换，模型列表一键拉取。用的是你自己的 AI 账号额度，不经过任何中间服务器，API Key 在本机加密保存。
- 纯文本模型也能用：截图先过本地 Windows OCR，变成文字再发过去。
- 框住一段抠不出来的文字，点一下取字，文字就在剪贴板里了——全程本地 OCR，不走网络。
- 可选的联网搜索（默认关闭）：打开后 AI 能查网页再回答。优先用服务商自带的联网（Claude / Gemini / OpenRouter）；其他服务商可在设置里切到应用内搜索（必应/Tavily），AI 搜了什么、读了哪个网页全程实时显示，本机和内网地址禁止访问。
- 框好的区域（连同你画的标注）可以一键钉在屏幕上：随手拖、滚轮缩放、要几个钉几个。
- 一个小色环跟着鼠标走，演示、录屏时观众一眼能跟上你的光标。半径、颜色、透明度都在设置里，截图永远拍不到它。
- 升级不用跑 GitHub：托盘菜单会自己冒出「更新到 vX.Y.Z」，点一下，下载、验签、安装一气呵成。
- 跑在 Windows 10/11 自带的 WebView2 上，装完约 11 MB。

### 安装

从 [Releases](../../releases) 下载 `AI Lens_x.y.z_x64-setup.exe`，运行即可。

> Windows 可能弹一个蓝色的「Windows 已保护你的电脑」——这是因为独立开发者没买几百刀一年的代码签名证书，不是程序有问题。点「更多信息 →仍要运行」即可。不放心的话，下面有从源码自己构建的方法。

### 先配一个 AI 服务

AI Lens 自己不带 AI，它把你的问题转发给你自己的 AI 账号。第一次打开会自动弹出设置，填三样东西：

1. **服务商**——用 ChatGPT 选 OpenAI，用 Claude 选 Anthropic，国内常用 DeepSeek（选 OpenAI 兼容）。
2. **API Key**——去服务商的控制台免费创建一个，复制粘贴进来。设置里有直达链接：
   - OpenAI：<https://platform.openai.com/api-keys>
   - DeepSeek：<https://platform.deepseek.com/api_keys>
   - Anthropic：<https://console.anthropic.com/settings/keys>
   - Google Gemini：<https://aistudio.google.com/apikey>
3. **模型**——点「拉取」从账号里选一个，或直接填（如 `gpt-4o`、`deepseek-chat`）。

> API Key 像一把钥匙，按用量从你的 AI 账号扣费——别分享给别人。它只存在你这台电脑上，且经 Windows DPAPI 加密。

### 用法

1. **Ctrl + Shift + S** —— 屏幕压暗，拖动框选。想换键去设置里改。
2. 工具栏里挑个工具在选区上画，或者直接打字。
3. **回车**发送，答案原地流式展开，可以接着追问。
4. **Esc** 或右键退出。

设置在托盘图标里：服务商、API Key、模型——点**拉取**直接从列表里挑。

### 未来计划

- [ ] 剪贴板里的图、拖进来的图也能直接问
- [ ] 托盘里翻看历史会话
- [ ] 标注工具栏加文字、高亮、马赛克
- [ ] 框选外文，译文原地盖上去
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
- Talks to OpenAI, DeepSeek, Ollama, OpenRouter, Azure, Anthropic, and Gemini. Switch in Settings; one click pulls the provider's model list. It uses your own AI account's quota — nothing routes through a middleman server, and your API key is stored encrypted on your machine.
- Text-only model? The screenshot runs through Windows OCR first and arrives as text.
- Frame some un-copyable text and hit the OCR button — it lands in your clipboard, fully local, nothing leaves the machine.
- Optional web search (off by default): let the AI look things up before answering. Provider-native search where available (Claude / Gemini / OpenRouter), or an in-app mode (Bing/Tavily) for everyone else — every search and page read is shown live, and local/LAN addresses are blocked.
- Pin the selection (your annotations included) right where it sits: drag it around, zoom with the wheel, pin as many as you like.
- A small colored ring rides along with your cursor, so viewers can follow your pointer in demos and recordings. Radius, color and opacity sit in Settings — and it never appears in your screenshots.
- Updates come to you: the tray menu flags a new version, and one click downloads, verifies and installs it.
- Runs on the WebView2 already inside Windows 10/11 — install lands at ~11 MB.

### Install

Grab `AI Lens_x.y.z_x64-setup.exe` from [Releases](../../releases) and run it.

> Windows may show a blue "Windows protected your PC" prompt. That's because a solo developer hasn't paid the few-hundred-dollars-a-year for a code-signing certificate — it doesn't mean the app is unsafe. Click "More info → Run anyway". If you'd rather not, build from source (below).

### Set up an AI service first

AI Lens has no AI of its own — it forwards your questions to your own AI account. On first launch it pops open Settings; fill in three things:

1. **Provider** — pick OpenAI if you use ChatGPT, Anthropic for Claude, or OpenAI-compatible for DeepSeek and most others.
2. **API Key** — create one (free) in the provider's console and paste it in. Settings links straight to each:
   - OpenAI: <https://platform.openai.com/api-keys>
   - DeepSeek: <https://platform.deepseek.com/api_keys>
   - Anthropic: <https://console.anthropic.com/settings/keys>
   - Google Gemini: <https://aistudio.google.com/apikey>
3. **Model** — hit "Pull" to pick one from your account, or just type it (`gpt-4o`, `deepseek-chat`, …).

> An API key is like a key that bills your AI account per use — don't share it. It never leaves this machine and is stored encrypted via Windows DPAPI.

### Usage

1. **Ctrl + Shift + S** — screen dims, drag to select. The hotkey lives in Settings if you want a different one.
2. Pick a tool from the toolbar and mark up the selection, or skip straight to typing.
3. **Enter** sends. The answer streams in place; keep asking.
4. **Esc** or right-click leaves.

Settings sit in the tray icon: provider, API key, model — hit **Pull** and pick from the list.

### Roadmap

- [ ] Ask about an image from the clipboard or a dropped file
- [ ] Browse past conversations from the tray
- [ ] Text labels, highlight and mosaic in the annotation toolbar
- [ ] Translate the selection and overlay the result in place
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
