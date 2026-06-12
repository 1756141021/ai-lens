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
  crypto.rs    — DPAPI seal/unseal for the API key at rest (raw crypt32 FFI)
  cursor.rs    — cursor highlight ring: follower thread (raw Win32 FFI) + ring window
  pin.rs       — pinned screenshots: in-memory PinStore + pin window creation

src/
  main.ts                 — routes by window label → Overlay or SettingsWindow
  windows/Overlay.svelte  — THE centerpiece: select + annotate + in-place chat (transparent overlay)
  windows/SettingsWindow.svelte — settings host
  windows/CursorRing.svelte     — the highlight ring (one div, restyled via config-changed)
  windows/PinWindow.svelte      — pinned screenshot (img + drag + wheel zoom + hover-✕)
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
- **Detach-on-send (floating answer panel)**: the first send shrinks the reused fullscreen
  overlay window onto the ask stack so the desktop is usable while slow answers stream. Two-step
  swap avoids flicker: (1) same frame — hide the canvas and pin the stack at explicit
  fullscreen-viewport coords; (2) setPosition+setSize back-to-back, then switch the stack to
  `inset:0` when the DOM `resize` event lands (300ms timeout fallback) — the Tauri promises
  resolve on IPC, not on webview reflow. Geometry: stack rect (CSS px) × dpr + `meta.originX/Y`
  (physical, can be negative). Gotchas baked in: `maximizable(false)` on the overlay builder
  (drag-region double-click triggers Tauri's built-in toggle-maximize), `.msgs` renders
  unconditionally in detached mode (otherwise the pendingSend gap leaves a transparent hole
  between strip and bar), model menu max-height clamps to `100vh` (window IS the viewport).
  Right-click-to-hide dies naturally (handler lives on the hidden canvas) — which also enables
  right-click text copy in the panel. No focus calls in the detach path: the input keeps DOM
  focus for an immediate follow-up, and nothing reclaims foreground after the user clicks away.
- **取字 + 钉图 (pin.rs / PinWindow.svelte)**: 取字 reuses capture_region's PNG path → async
  `ocr_image` → clipboard (the command went async because a sync command runs on the MAIN thread
  — WebView2 raises IPC there — and the 1-2s PowerShell froze every window). 钉图 holds the
  image base64 in an in-memory `PinStore` keyed by window label (NOT a cache path —
  `cache::cleanup` can't delete the file under a pin), one `pin-{n}` window per pin, cleaned up
  on `WindowEvent::Destroyed`. **Window creation from commands — hard-won rules**: a SYNC
  command must NEVER build a webview window — it executes inside the WebView2 IPC handler and
  re-enters the message loop; the IPC response never returns and every later invoke from the
  calling window hangs (looked like "Esc/right-click are dead"). An async command building
  directly from the tokio pool ALSO hung silently in practice (despite source-reading suggesting
  the proxy enqueue is fine). The only pattern that works: **async command +
  `run_on_main_thread`** with an mpsc channel for the result (pin.rs) — same hop run_capture
  uses. Pin page: `data-tauri-drag-region="deep"` makes any descendant
  drag the window while BUTTONs are exempt automatically (drag.js isClickableElement) — the
  hover-✕ needs no pointer-events tricks. 1:1 sizing = physical position/size post-build +
  img at 100vw/vh; wheel zoom rescales the window (0.25–3×) from the stored physical dims. answer + per-code-block copy buttons. WebView2 routes
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

## Security (0.9.0 hardening)

- **Model output is untrusted input.** A screenshotted page can prompt-inject the model into
  emitting live HTML. `renderMd` (Overlay.svelte) is the single markdown→HTML point and wraps
  `marked.parse` in `DOMPurify.sanitize`. The custom code-block renderer's output (`.cb` div +
  `.cbcopy` button + hljs spans + inline SVG) survives DOMPurify's default allowlist — verified
  live with `<img onerror>` / `<script>` payloads stripped and copy buttons still working.
- **CSP** (`tauri.conf.json`): `script-src 'self'`, `connect-src 'self' ipc: http://ipc.localhost`
  (Tauri IPC on Windows goes over `http://ipc.localhost` — omit it and EVERY invoke dies),
  `img-src 'self' data: blob:` (pins + crops are data URLs), `style-src 'unsafe-inline'` (Svelte).
  LLM traffic is unaffected: it runs through the Rust http plugin, not webview fetch.
  **PITFALL: CSP only exists in the built app.** Dev pages come from Vite at `localhost:1420`
  and Tauri can't inject headers there — CSP regressions are invisible in `tauri dev`; test
  against `pnpm tauri build --no-bundle` + the release exe (CDP attaches the same way, pages
  live at `http://tauri.localhost/`).
- **API key at rest**: `save_config` seals `api.api_key` with DPAPI (CURRENT_USER) →
  `dpapi:<base64>` in config.json; `load_config` unseals, silently upgrades legacy plaintext on
  first load, and on unseal failure (config copied from another machine/user) clears the key so
  first-run onboarding reopens. Memory + IPC stay plaintext. `protect()` failure falls back to
  plaintext write — a working app beats a locked-out one. Interop sanity check: PowerShell
  `[Security.Cryptography.ProtectedData]::Unprotect()` opens the same blobs.
  **Downgrade caveat**: ≤0.8.0 reads `dpapi:…` as the literal key → 401s until re-entered.
- Threat-model note for the key: `%APPDATA%` already has per-user ACLs, so other local users
  were never the issue; DPAPI guards against the config file traveling (cloud sync, backups,
  hand-shared configs).
- **API errors are humanized in one place** — `humanizeHttpError` (api.ts), used by both
  `streamChat` and `fetchModels`: plain-Chinese first line per status
  (400/401/402/403/404/408/413/422/429/5xx), raw status+body+URL below; `.err`/`.mierr`/`.hint.err`
  render multi-line via `white-space: pre-line`.
- **Navigation guard on the overlay** (0.9.1): DOMPurify keeps http(s) `<a>` links in answers,
  and a click on one used to NAVIGATE the overlay webview off index.html — a reused window
  navigated away is bricked (next capture only repositions + emits `capture-ready`; nobody is
  listening anymore). `build_overlay` sets `on_navigation`: own origins pass (`*.localhost`,
  dev `localhost`), external http(s) returns false and opens via `app.opener().open_url` instead
  (tauri-plugin-opener — `shell.open` is deprecated; the shell plugin was dropped entirely,
  Settings key links use `plugin:opener|open_url` + `opener:default` capability).
  Covers left/middle click and `location.href=`. `target=_blank`/`window.open` need no handling:
  wry's NewWindowRequested defaults to `SetHandled(true)` (suppressed) when no handler is set.
  Only the overlay renders untrusted markdown — pin renders one `<img>`, settings/cursor render
  only our own UI — so the guard lives on that one builder.

## Auto-update & releases

- **Signing keys**: `.signing/ai-lens.key` (private) + `key-password.txt` + `.pub` — the whole
  dir is gitignored. The pubkey is embedded in `tauri.conf.json` (`plugins.updater.pubkey`).
  **Back the private key + password up OUTSIDE the repo** — lose them and existing installs can
  never accept another update. Regenerate: `pnpm tauri signer generate -w .signing/ai-lens.key -p <pw> -f`.
  Windows gotcha: an empty env var is deleted by PowerShell, so a passwordless key can't be
  used unattended — that's why the key HAS a password, stored next to it.
- **Update flow (app side, `updater.rs`)**: tray item 检查更新; silent startup check
  (release builds only) renames it to 更新到 vX.Y.Z; click → download (progress in menu text,
  signature verified) → NSIS `/UPDATE` passive install → installer relaunches the app
  (`install()` exits the process — code after `download_and_install` is unreachable on Windows).
- **Release**: bump version in tauri.conf.json + Cargo.toml + package.json, update CHANGELOG,
  commit, then `pwsh scripts/release.ps1` — it builds signed artifacts, writes `latest.json`
  (GitHub turns spaces in asset names into DOTS — the script accounts for it), and
  `gh release create` with setup.exe/.sig/.msi/latest.json. `latest.json` must be on the
  LATEST release or `releases/latest/download/latest.json` 404s (updater treats it as no-op).
- **PITFALL — never hand-roll a release.** A manual `gh release create` without `latest.json`
  (or with an unsigned build) silently breaks auto-update for every installed user: their tray
  check sees nothing, or the download fails signature verification. `scripts/release.ps1` is the
  only supported release path.
- **MSI caveat**: auto-update always installs the NSIS payload, so MSI-installed users migrate
  to the NSIS lineage (stale entry in Add/Remove). MSI stays a manual-download option.
- **Local E2E without publishing**: serve a fake-newer `latest.json` + signed setup.exe via
  `python -m http.server 8765`, run `pnpm tauri dev -- --config dev-update.json` (overrides
  endpoints to localhost; `dangerousInsecureTransportProtocol` permits http). `Update::download()`
  verifies the signature without installing.

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
- **CDP is the reliable way to drive the UI** (physical mouse fights the human at the machine):
  restart dev with `$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"`,
  list pages via `http://127.0.0.1:9222/json` (overlay/cursor/settings share the same URL —
  identify by DOM probes), then `Runtime.evaluate` over the WebSocket: synthesize the selection
  drag with `canvas.dispatchEvent(new MouseEvent(...))` in CSS px (no DPI math), click buttons
  with `.click()`, read state from the DOM, and call commands via
  `window.__TAURI_INTERNALS__.invoke(...)`. Physical-pixel automation needs
  `SetProcessDPIAware()` first (150% scaling here) and an idle-gate (`GetLastInputInfo`) so the
  user's mouse doesn't race the script.
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
- [x] First-run onboarding (no API key → Settings opens itself)
- [x] 开机自启 toggle (tauri-plugin-autostart, OS-backed — not in config.json)
- [x] In-app auto-update (tray menu, signed GitHub Releases, see Auto-update & releases)
- [x] Detach-on-send: answer streams in a draggable floating panel, desktop stays usable
- [x] 取字: selection → local OCR → clipboard, one click
- [x] 钉图: pin the (annotated) selection 1:1 on screen — drag / wheel zoom / multi-pin
- [x] 0.9.0 hardening: DOMPurify on model output, CSP, DPAPI-sealed API key (see Security)
- [x] 0.9.0 onboarding: humanized API errors, per-provider key links + first-run banner in
      Settings, README SmartScreen note + key walkthrough (from the three-persona review)
