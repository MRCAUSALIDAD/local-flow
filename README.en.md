# Local Flow

**🌐 Language:** [Español](README.md) · **English**

**100% local and private** voice dictation for macOS, Windows and Linux.
Hold a key, speak, release — the transcribed text is typed straight into
whatever app you're in (and copied to the clipboard).

## 🔒 Privacy: everything runs locally

- **Audio never leaves your computer.** It is captured, transcribed and
  discarded on your own machine.
- **No cloud, no servers, no telemetry.** Nothing is sent to any external or
  third-party service.
- Transcription uses **Whisper (whisper.cpp)**, an AI model that runs
  **locally** on your CPU.
- **Only internet use:** downloading the voice model once from Hugging Face.
  After that it works fully **offline**.

---

## 1. Prerequisites

You need three base things on any system:

- **Node.js 18+** and npm — https://nodejs.org
- **Rust** (stable toolchain) — https://rustup.rs
- **C/C++ toolchain + CMake** (whisper.cpp is compiled on your machine).

### macOS

```sh
xcode-select --install          # C/C++ build tools
brew install cmake node rustup-init && rustup-init
```

### Windows

- Install **Microsoft C++ Build Tools** ("Desktop development with C++" workload).
- Install **CMake** — https://cmake.org/download (check "Add to PATH").
- Install **Node.js** and **Rust** (rustup).
- **WebView2**: already bundled on Windows 11 (on Windows 10 the app installs it).

### Linux (Debian/Ubuntu)

```sh
sudo apt update
sudo apt install -y build-essential cmake curl file \
  libasound2-dev libxdo-dev \
  libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev \
  libgtk-3-dev libayatana-appindicator3-dev
# Node and Rust: install from nodejs.org and rustup.rs
```

> `libasound2-dev` = microphone (cpal) · `libxdo-dev` = typing into other apps
> (enigo) · the rest = Tauri/WebKit.

---

## 2. Install and run

```sh
# 1. Clone the repository
git clone <repo-url> local-flow
cd local-flow/apps/desktop

# 2. Install frontend dependencies
npm install

# 3a. Development mode (live reload)
npm run tauri dev
```

The first Rust build takes several minutes (it compiles whisper.cpp);
later builds are fast thanks to caching.

### Build the installable app

```sh
npm run tauri build
```

Output in `src-tauri/target/release/bundle/`:

- **macOS:** `macos/Local Flow.app` (drag it to `/Applications`) and
  `dmg/Local Flow_*.dmg`
- **Windows:** `msi/` or `nsis/` (`.exe` installer)
- **Linux:** `deb/`, `rpm/` or `appimage/`

---

## 3. Download the voice model

When you first open the app:

1. Go to **Settings → Voice model**.
2. Press **Download** on a model:
   - **Tiny** (~75 MB) — fastest, lowest accuracy.
   - **Base** (~142 MB) — recommended, balanced.
   - **Small** (~466 MB) — slower, best accuracy.
3. It downloads once, activates automatically, and the status flips to
   **Model ready**.

From here it works **without internet**.

---

## 4. Managing permissions

### macOS

Two permissions, both in *System Settings → Privacy & Security*:

1. **Microphone** — requested on the first dictation. Allow it.
2. **Accessibility** — required to type into **other** apps.
   - The app shows a banner with a button to open the pane, or go to
     *Privacy & Security → Accessibility* and **enable Local Flow**.

> **Important (unsigned builds):** every time you **rebuild** the app, macOS may
> invalidate the Accessibility permission even if the checkbox stays checked.
> If it stops typing into other apps: remove Local Flow from the list with **–**,
> add it again with **+**, and restart the app.

> **Gatekeeper:** on the first launch of an unsigned build, right-click the app
> → **Open**, or run:
> `xattr -dr com.apple.quarantine "/Applications/Local Flow.app"`

### Windows

- No special permission is needed to type into other apps.
- Windows Defender SmartScreen may warn about an unsigned app:
  **More info → Run anyway**.

### Linux

- **X11** sessions: works directly.
- **Wayland** sessions: keyboard injection (enigo) and global shortcuts may be
  limited by the compositor; if typing into other apps fails, use the clipboard
  (on by default) and paste manually.

---

## 5. Usage

- **Hold `⌥ Space`** (Alt+Space) in any app → speak → release.
- The text is **typed where your cursor is** and copied to the clipboard.
- **Floating indicator dot** (always on top): green = active, pulsing cyan =
  dictating, amber = transcribing.
- **Metrics:** hover over the dot to see CPU, RAM, audio duration, transcription
  time and speed. In *Settings* you can **pin** them to a corner.
- Closing the window sends it to the **system tray**; reopen it from there.

---

## 6. Uninstall

- **macOS:** delete `/Applications/Local Flow.app` and
  `~/Library/Application Support/com.gabriel.local-flow` (models and config).
- **Windows:** uninstall from *Apps*; delete `%APPDATA%\com.gabriel.local-flow`.
- **Linux:** remove the package; delete `~/.local/share/com.gabriel.local-flow`.

---

## Architecture

```
UI (React)  ──invoke/events──►  Backend (Rust)
                                 ├── audio.rs    cpal capture + resample→16kHz
                                 ├── whisper.rs  whisper.cpp transcription (local AI)
                                 ├── models.rs   model download/management
                                 ├── config.rs   JSON settings
                                 └── lib.rs      hotkey · tray · overlay · metrics
```

Third-party licenses: see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
