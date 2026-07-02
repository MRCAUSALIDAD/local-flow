# Local Flow

Offline, private, low-latency voice dictation for macOS / Windows / Linux.
Hold a hotkey, speak, release — the transcript lands on your clipboard (and
optionally auto-pastes into the focused app). Everything runs locally via
whisper.cpp; no audio ever leaves the machine.

> MVP scope: Fases 1–2 of the spec (hotkey → capture → transcribe → clipboard /
> paste, plus settings). Local LLM post-processing (Fase 3) is intentionally
> out of scope for now.

## Stack

- **Frontend:** Tauri v2 · React + TypeScript · Vite
- **Backend:** Rust · cpal (audio) · whisper-rs (whisper.cpp) · enigo (paste)
- **System:** global shortcut · clipboard · system tray

## Prerequisites

- Node.js + npm
- Rust toolchain (`rustup`)
- A C/C++ toolchain + **cmake** for whisper.cpp (Xcode CLT on macOS; build-essential + cmake on Linux)

## Run

```sh
npm install
npm run tauri dev
```

Then open **Settings → Voice model** and click **Download** on a model
(Tiny / Base / Small). It downloads once into the app data dir, activates
automatically, and the status chip flips to **Model ready** — no manual files.

## Build (installable app)

```sh
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`:
- `macos/Local Flow.app` — drag to `/Applications`
- `dmg/Local Flow_0.1.0_aarch64.dmg` — installer

The app icon is generated from `src-tauri/app-icon.svg` via `npm run tauri icon
src-tauri/app-icon.svg` (re-run if you edit the logo).

> Unsigned build: first launch, right-click the app → **Open** to bypass
> Gatekeeper (or `xattr -dr com.apple.quarantine "/Applications/Local Flow.app"`).

## Use

- **Hold `⌥ Space`** anywhere (app just needs to be running) → speak → release.
- Transcript is **typed into the focused app** at your cursor (and copied to the
  clipboard). A floating pill shows while listening.
- Close hides to the **menubar tray**; reopen via tray icon or dock.

### macOS permissions

- **Microphone** — prompted on first dictation (allow it).
- **Accessibility** — required to type into other apps. Home banner → **Grant**,
  or *System Settings → Privacy & Security → Accessibility* → enable Local Flow.

## Architecture

```
UI (React)  ──invoke/events──►  Backend (Rust)
                                 ├── audio.rs    cpal capture + resample→16kHz
                                 ├── whisper.rs  whisper.cpp transcription
                                 ├── config.rs   persisted JSON settings
                                 └── lib.rs      hotkey · tray · clipboard · flow
```

Events emitted to the UI: `flow-status` (`idle` | `listening` | `transcribing`),
`flow-transcript`, `flow-error`.
