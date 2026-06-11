# Changelog

All notable changes to AI Lens are recorded here.

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
