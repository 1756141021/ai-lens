# Changelog

All notable changes to AI Lens are recorded here.

## [0.9.2]

First-session user feedback round.

### Added
- **Add a screenshot to an ongoing conversation** — press the capture hotkey while a chat panel is open, frame a region as usual, and a small chooser pops up at the cursor: **新问题** (start over) or **追加** (attach this capture to the current conversation as the next question's image). With no conversation open, capture behaves exactly as before. Esc / right-click in the chooser returns to the conversation — and backing out of a 新问题 before sending restores the old conversation too (it's only discarded once the new question is actually sent).
- **Question screenshot in the strip, click to enlarge** — the floating panel's top strip shows a thumbnail of the captured region next to the current question; click it for a full-size preview over the panel (click / Esc to close).

### Fixed
- **Shift+Enter now inserts a newline** — the ask bar was an `<input>` (single-line by nature); it's a `<textarea>` now, auto-growing up to ~5 lines. Enter still sends.
- **Links in answers were eye-searing** — `{@html}` content carried the browser's default saturated blue/purple on the dark glass panel. Links now use the app's soft blue, and long URLs wrap instead of overflowing.

### Changed
- **The floating panel's top strip now shows the current question** — it used to be an unlabeled empty bar (just ✕ + an invisible drag region); now the question you asked stays readable there even after the answer scrolls it away. Dragging and ✕ behave as before.

## [0.9.1]

### Fixed
- Clicking a link inside an AI answer navigated the overlay webview away from the app — and since the overlay window is prewarmed and reused, every later capture hit a dead page until the app was restarted. An `on_navigation` guard now keeps the overlay on its own origin and diverts external http(s) links to the system browser. Left/middle click and script-driven navigation are all covered.

## [0.9.0]

Hardening + onboarding round, driven by a three-persona review (junior dev / staff engineer / non-technical user).

### Security
- **Markdown sanitization** — model output now passes through DOMPurify before rendering. A screenshotted page can carry a prompt injection that makes the model emit live HTML (`<img onerror=…>`, `<script>`); that HTML used to land in the webview unsanitized, with IPC in reach. Verified end-to-end: payloads stripped, normal markdown/code blocks/copy buttons intact.
- **Content-Security-Policy enabled** (was `null`): `script-src 'self'`, external `connect-src` blocked, images limited to self/data/blob. Defense in depth behind the sanitizer. Note: CSP only applies to the built app — the dev server (Vite-served pages) runs without it.
- **API key encrypted at rest** — `config.json` now stores the key DPAPI-sealed (`dpapi:` prefix, CURRENT_USER scope). Legacy plaintext configs upgrade automatically on next launch; in-memory and IPC stay plaintext. A config copied to another machine/user can't be decrypted — the key is cleared and onboarding reopens. **Downgrading to ≤0.8.0 after this requires re-entering the key.**

### Added
- **Human-readable API errors** — 400/401/402/403/404/408/413/422/429/5xx now lead with what to do in plain Chinese ("API Key 无效或没有权限——去设置里检查…"), with the raw status/body kept underneath for debugging.
- **API Key guidance in Settings** — a per-provider "create a key here" link row under the API Key field (OpenAI / DeepSeek / OpenRouter / Anthropic / Google AI Studio), opening in the system browser.
- **First-run welcome note** — when no key is configured yet, Settings opens with a short banner: the app lives in the tray, and the hotkey to press once configured.
- **README onboarding** — SmartScreen heads-up for the unsigned installer, a "set up an AI service first" walkthrough with key-console links, and a plain-words note that requests use your own account with no middleman.

## [0.8.0]

### Added
- **取字** — a toolbar button OCRs the selection (local WinRT) and puts the text straight into the clipboard; the icon flashes ✓/✗.
- **钉图** — pin the selection (annotations baked in) exactly where it is, as a 1:1 always-on-top frameless window. Drag anywhere to move, mouse-wheel to zoom (25%–300%), hover-✕ or Esc to close; multiple pins coexist.

### Changed
- The OCR command is now async — it used to run its 1-2s PowerShell call on the main thread, freezing every window meanwhile.

## [0.7.0]

### Added
- **Send and keep working** — the moment you send your question, the fullscreen dim vanishes and the overlay shrinks into a small draggable always-on-top panel next to your selection. The answer streams into the panel while the rest of the desktop is fully usable; follow up in the panel, close with Esc or ✕. Slow models no longer hold your screen hostage.

## [0.6.0]

### Added
- **In-app auto-update** — the tray menu gains 检查更新. A silent startup check renames it to "更新到 vX.Y.Z" when a release is newer; clicking downloads the signed NSIS package (progress in the menu text), verifies the signature, and installs passively with an automatic relaunch. Updates are pulled from this repo's GitHub Releases (`latest.json`); signing keys live locally in the gitignored `.signing/`.

## [0.5.0]

### Added
- **First-run onboarding** — launching with no API key configured opens Settings automatically.
- **开机自动启动** toggle in Settings (off by default, via tauri-plugin-autostart).

### Changed
- 光标高亮 settings section is collapsed by default, same style as 高级.

## [0.4.0]

### Added
- **Cursor highlight ring** — a small translucent circle follows the mouse while the app sits in the tray, like the cursor highlight in recording tools. Settings: enable/disable, radius, opacity, color. The ring window is click-through, never takes focus, and is excluded from screen capture (`WDA_EXCLUDEFROMCAPTURE`), so it never shows up in your screenshots.
- "AI 生成，请自行甄别" disclaimer line under answers.

## [0.3.0]

### Added
- **One-click copy** — hover an answer to copy it as markdown; every code block gets its own copy button. Clipboard writes go through the official clipboard-manager plugin (WebView2's `navigator.clipboard` needs a permission grant Tauri doesn't auto-handle).
- **Quick prompts** — 解释 / 翻译 / 总结 chips above the input bar, shown until the first message; one click sends with the selection attached.
- **Model switch in the ask bar** — a model pill next to the input opens the persisted model list; picking one saves it for future captures. The list can also be pulled right from the overlay. Pulled models now persist in config (`api.models`).

### Fixed
- HTTP scope `http://*` / `https://*` only matched default ports, so custom-port endpoints — local Ollama (`localhost:11434`) included — were rejected with "url not allowed". Scope is now `*:*`.
- Stop button did nothing during streaming when the input was empty (the empty-input check ran first).
- Settings window is hidden and reused, so it showed stale values after the overlay saved a model switch — pressing 保存 there would have reverted it. It now reloads on `config-changed`.

## [0.2.0]

### Added
- **Multi-provider switching** — OpenAI-compatible (OpenAI / DeepSeek / Ollama / OpenRouter / Azure), Anthropic (native `/v1/messages`), and Google Gemini (native `streamGenerateContent`). Picked in Settings; base URL may be blank to use the official endpoint.
- **Annotation on the selection** — arrow / rectangle / freehand pen + color + undo. Annotations are baked into the image sent to vision models.
- **Model pull** — Settings "拉取" fetches the provider's model list into a dropdown.
- New app icon (camera aperture).

### Changed
- **Capture is now a transparent overlay over the live desktop** (QQ-style): a dim layer with the selection punched through as a transparent hole. Removes the per-capture full-screen JPEG encode entirely — capture went from ~700ms–1s to near-instant.
- Overlay window is prewarmed once at startup and reused every capture (no per-capture webview rebuild).
- Requests now route through the **Tauri HTTP plugin** instead of raw webview `fetch`, which the webview's CORS was blocking — the symptom was an opaque "Failed to fetch". API errors now surface the real status, body, and endpoint.

### Fixed
- First question could be sent without the image if you typed and submitted before the crop finished — submit now waits for the crop.
- Capture latency (see Changed — transparent overlay).

## [0.1.0]

- Initial Tauri v2 + Svelte 5 scaffold: tray app, hotkey → region capture → AI chat, OAI-compatible streaming, Windows OCR fallback, screenshot-cache cleanup, configurable hotkey, multi-monitor.
