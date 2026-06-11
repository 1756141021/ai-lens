# Changelog

All notable changes to AI Lens are recorded here.

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
