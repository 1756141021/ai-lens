# AI Lens — DEV_NOTES

## What is this

Lightweight Windows screenshot-to-AI tool. Global hotkey → freeze-select a screen
region → annotate it → ask AI about it, answer streams in place over the screenshot.

## Tech Stack

- **Tauri v2** (Rust backend, WebView2 frontend), tray-only (no main window)
- **Svelte 5** runes + Vite
- **Multi-provider**: OpenAI-compatible / Anthropic (native) / Google Gemini (native), switchable
- Streaming SSE via **Tauri HTTP plugin** (routes through Rust to bypass webview CORS)
- Windows WinRT OCR via PowerShell (for non-vision / text-only models)

## Project Layout

```
src-tauri/src/
  lib.rs       — tray + prewarmed transparent `overlay` window + `settings` window, commands
  capture.rs   — xcap capture (monitor under cursor) to memory + region crop
  ocr.rs       — Windows OCR via PowerShell WinRT call
  cache.rs     — screenshot-cache cleanup (keeps newest N .png)
  config.rs    — config read/write, live hotkey re-register, Tauri commands
  cursor.rs    — cursor highlight ring: follower thread (raw Win32 FFI) + ring window

src/
  main.ts                 — routes by window label → Overlay or SettingsWindow
  windows/Overlay.svelte  — THE centerpiece: select + annotate + in-place chat (transparent overlay)
  windows/SettingsWindow.svelte — settings host
  windows/CursorRing.svelte     — the highlight ring (one div, restyled via config-changed)
  components/Settings.svelte    — settings form (provider dropdown, model pull, advanced)
  lib/api.ts              — provider adapters, streamChat, fetchModels
  lib/chat.svelte.ts      — chat state (Svelte 5 runes)
  lib/config.ts           — config bridge to Rust
```

## Capture flow

```
[hotkey / tray] → run_capture (lib.rs):
    capture::grab — capture monitor under cursor into memory (LastCapture), store geometry.
                    NO encode (overlay is transparent, shows the live desktop).
    reuse the prewarmed overlay → set_position/set_size over that monitor → emit "capture-ready"
[overlay listens "capture-ready"] reset state → draw dim + show window (frontend shows it)
[select] drag → selRect; mouseup → capture_region crops LastCapture (PNG) for the AI image
[annotate] arrow/rect/pen on the canvas over the selection
[ask] type → streamChat (frontend, via http plugin); answer streams into the in-place glass stack
[Esc / right-click] hide the overlay (reused next capture; state reset on next "capture-ready")
```

## Architecture Decisions

- **Tray-only, prewarmed transparent overlay (QQ-style)**: No main window. One `overlay`
  window is built hidden at startup (`.transparent(true)`) and **reused** every capture — no
  per-capture webview rebuild. `draw()` paints a 45% black dim over the *live desktop* and the
  selection `clearRect`s a transparent hole (desktop at full brightness). There is **no frozen
  frame and no image encode on the hot path** — that's what makes it feel instant. The image
  actually sent to the AI is cropped from the in-memory snapshot taken at trigger time
  (`LastCapture`), so dynamic content is captured at the trigger instant even though the user
  sees live pixels while framing. This replaced an earlier opaque-overlay design whose JPEG
  encode of the full monitor cost ~700ms–1s (see Performance).
- **In-place ask**: after selection, a dark-glass stack (annotation toolbar + answer messages +
  input) is absolute-positioned anchored to the selection, clamped on-screen. Everything happens
  in the one overlay window — no second popup. (The earlier design popped a second window with
  hide/show/resize churn = visible flicker; single window + zero geometry changes = zero flicker.)
  Quick-prompt chips (解释/翻译/总结) sit between the toolbar and the input until the first
  message; they go through the same `send()` path as typing (awaits the crop, bakes annotations).
- **Copy via clipboard-manager plugin**: answer + per-code-block copy buttons. WebView2 routes
  `navigator.clipboard` through a PermissionRequested event Tauri doesn't auto-grant, so writes go
  through `tauri-plugin-clipboard-manager` (capability `clipboard-manager:allow-write-text`).
  Code blocks are `{@html}` markup — Svelte can't bind events inside, so the marked code renderer
  wraps each block as `.cb` + `.cbcopy` button and ONE delegated click handler on `.msgs` reads
  the sibling `pre code` `textContent` (highlight spans don't change textContent).
- **Model switch in the ask bar**: a pill next to the input opens an upward glass dropdown of
  `api.models` (persisted list — pulled in Settings or live from the overlay). Picking saves the
  whole config (`set_config` only re-registers the hotkey when it changed, so no side effects).
  The Settings window is hidden+reused, so it listens to `config-changed` and remounts its form —
  otherwise its stale Save would revert the overlay's pick.
- **Annotation**: arrow / rectangle / freehand pen + color + undo. Shapes are stored in CSS-px
  (same space as `selRect`) and drawn on the canvas, clipped to the selection. For vision sends
  with annotations, `composite()` bakes the shapes onto the **full-res clean crop** (not the
  on-screen render) → crisp. OCR (non-vision) uses the clean crop with no annotations (text
  recognition shouldn't see the arrows).
- **Multi-provider adapters** (`lib/api.ts`): `streamChat(config, turns, signal)` takes neutral
  `ChatTurn[]`; each provider builds its own wire format + parses its own SSE.
  - `openai` — `/chat/completions`, Bearer (Azure → `api-key`), `choices[].delta.content`.
    Covers OpenAI / DeepSeek / Ollama / OpenRouter / Azure.
  - `anthropic` — native `/v1/messages`, `x-api-key` + `anthropic-version` +
    `anthropic-dangerous-direct-browser-access`, `content_block_delta.delta.text`.
  - `gemini` — native `:streamGenerateContent?alt=sse`, `x-goog-api-key`,
    `candidates[0].content.parts[0].text`.
  - Base URL may be blank → provider's official endpoint. `api_version` doubles as Azure
    `api-version` / Anthropic `anthropic-version`.
- **HTTP plugin, not browser fetch (CORS)**: raw `fetch` from the webview origin
  (`tauri://localhost`) is CORS-blocked by LLM APIs → opaque "Failed to fetch". `api.ts` imports
  `fetch` from `@tauri-apps/plugin-http`, which routes through Rust (no CORS) and surfaces the
  real status + body. Capability scope: `http:default` allow `https://*` + `http://*` (URLPattern
  expands an empty path to `*`). The plugin's `fetch` streams chunk-by-chunk, so SSE still works.
- **Model pull** (`fetchModels`): the Settings "拉取" button hits the provider's models endpoint
  (`/models`, Anthropic `/v1/models`, Gemini `/v1beta/models`) and fills a `<select>`. A
  `<datalist>` was tried first but the browser filters its options by the input text — switched to
  a real `<select>`.
- **OCR via PowerShell**: the `windows` crate version conflicts with xcap, so WinRT OCR is called
  through a PowerShell subprocess. Path/language are interpolated into a single-quoted PS string
  and escaped by doubling `'` (backslashes are literal in PS single-quotes — do NOT escape them).
- **Cursor highlight ring** (`cursor.rs` + `CursorRing.svelte`): a tiny always-on-top,
  click-through (`set_ignore_cursor_events`), `focusable(false)` window whose webview is one
  circle div. A detached Rust thread polls `GetCursorPos` at ~125Hz and moves the window with raw
  `SetWindowPos` (FFI, no `windows` crate — the old xcap version conflict is gone from the
  lockfile but the no-dep policy stays). cx/cy are re-asserted every move instead of SWP_NOSIZE so
  WM_DPICHANGED resizes self-heal when crossing mixed-DPI monitors. `content_protected(true)`
  (tao → `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, Win10 19041+) keeps the ring out of
  xcap's GDI BitBlt grab — screenshots never contain it. DPI strategy: radius is config'd in
  PHYSICAL px, the window is sized physical, the div uses 100vw/vh — no scale math anywhere.
  Settings restyle live via the existing "config-changed" event; show/hide/resize via
  `cursor::apply_config_change` from `set_config`.
- **Rust module deps**: `capture → cache → config` is an acyclic shared-utility dependency (cache
  dir path, `AppConfig` for command state), not a layering violation. The Rust↔frontend boundary
  stays command/event-mediated.

## Performance (capture speed)

The hot path used to JPEG-encode the full monitor before showing the overlay. Findings:

- The `image` crate's pure-Rust JPEG encoder: **~700ms–1000ms** for a 4MP monitor (no SIMD).
- Swapping to the SIMD `jpeg-encoder` crate: ~415ms.
- Downscaling the preview before encode: ~160ms.
- **`[profile.dev]` opt-level=3 on our own crate** was the real unlock for the above — generic
  `image`-crate fns (`resize`, `encode`) **monomorphize into THE CALLING crate**, so
  `[profile.dev.package."*"]` alone doesn't optimize them (a hand-rolled RGBA→RGB loop hit ~900ms;
  an `imageops::resize` call hit ~1.4s). Remember this trap if image work ever returns to the hot
  path.
- Final answer: went **transparent overlay → zero encode**. The encode crates were removed; only
  PNG (for the crop, off the hot path) remains — so the dev opt-level overrides were reverted too
  (smaller `target/`, faster incremental builds). Only `[profile.release]` keeps strip/lto/opt-s.

## Config

`%APPDATA%/ai-lens/config.json` — auto-created on first run.

Key fields: `api.provider` (openai/anthropic/gemini), `api.base_url` (blank → official),
`api.api_key`, `api.model`, `api.models` (persisted pulled model list, fills the ask-bar
dropdown), `api.supports_vision` (true → send image incl. annotations; false → local OCR text),
`api.auth_header`, `api.api_version`, `hotkey`, `cache.max_count`, `ocr.language`,
`cursor.{enabled,radius,opacity,color}` (highlight ring; radius in physical px).

## Running

```
pnpm install
pnpm tauri dev
```

First `tauri dev` after a clean (or a profile/dependency change) recompiles all deps once.
Incremental Rust edits after that are seconds.

## Self-verification (no direct GUI access)

- Trigger the hotkey from PowerShell `keybd_event` (Ctrl+Shift+S), `SetCursorPos`/`mouse_event`
  to drag-select, then `CopyFromScreen` to screenshot the composited result (confirms the
  transparent overlay renders the dimmed live desktop + selection hole, not a black window).
- Test an endpoint over Rust's network path with PowerShell `Invoke-RestMethod` (same network as
  the http plugin, no CORS) to tell a config error apart from a provider outage.

## Status

- [x] Tray-only app, prewarmed transparent overlay, settings window
- [x] Hotkey → capture-under-cursor → select → in-place streaming chat, multi-turn
- [x] Annotation: arrow / rect / pen / color / undo, baked into the vision image
- [x] Multi-provider: OpenAI-compatible / Anthropic native / Gemini native, switchable
- [x] Model pull (Settings 拉取 → provider models endpoint → dropdown)
- [x] HTTP plugin (CORS bypass) + real error messages
- [x] Vision / OCR fallback, configurable hotkey, multi-monitor, screenshot-cache cleanup
- [x] Capture speed: zero-encode transparent overlay (was ~1s JPEG encode)
- [x] One-click copy: whole answer + per-code-block (clipboard-manager plugin)
- [x] Quick prompts on the input bar (解释/翻译/总结)
- [x] Model switch from the ask bar (persisted `api.models`, in-overlay pull)
- [x] Cursor highlight ring (click-through follower window, capture-excluded, settings-tunable)
- [x] AI disclaimer line under answers
