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

## ⬇️ Install

No build toolchain, no Rust: just download a prebuilt installer.

### One-line installer

**macOS and Linux**

```sh
curl -fsSL https://raw.githubusercontent.com/MRCAUSALIDAD/local-flow/main/scripts/install.sh | bash
```

**Windows** (PowerShell)

```powershell
irm https://raw.githubusercontent.com/MRCAUSALIDAD/local-flow/main/scripts/install.ps1 | iex
```

The script detects your OS and architecture, downloads the installer from the
latest release and installs it (on macOS it also clears the Gatekeeper
quarantine flag).

### Manual download

From [**Releases**](https://github.com/MRCAUSALIDAD/local-flow/releases/latest):

| System | File | How to install |
| --- | --- | --- |
| macOS (Apple Silicon and Intel) | `..._universal.dmg` | Open, drag to *Applications* |
| Windows | `..._x64-setup.exe` | Run the installer |
| Debian/Ubuntu | `..._amd64.deb` | `sudo apt install ./<file>.deb` |
| Other distros | `..._amd64.AppImage` | `chmod +x` and run |

> Builds are **unsigned** (no developer certificate).
> macOS: right-click → **Open**, or
> `xattr -dr com.apple.quarantine "/Applications/Local Flow.app"`.
> Windows: SmartScreen → **More info → Run anyway**.

After installing, continue with [step 3: download the voice model](#3-download-the-voice-model).

---

## 1. Prerequisites (only to build from source)

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

Three permissions, all in *System Settings → Privacy & Security*:

1. **Microphone** — requested on the first dictation. Allow it.
2. **Accessibility** — required to type into **other** apps.
   - The app shows a banner with a button to open the pane, or go to
     *Privacy & Security → Accessibility* and **enable Local Flow**.
3. **System audio capture** — needed for the *Listen* tab. It is a separate
   permission category from the microphone. In testing on macOS 26 capture
   worked without any prompt; if your system does ask, allow it.

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

## 6. Listening to your computer's audio

Beyond dictation, Local Flow can transcribe **whatever your computer is
playing**: a video call, a YouTube video, a podcast. Still fully local.

1. Open the **Listen** tab.
2. Leave **Audio source** on *Automatic* (or pick a specific output).
3. Press **Start listening** and play whatever you want transcribed.

Text appears live, labelled by speaker: **Them** (what the computer plays) and
**Me** (your microphone, if you leave that track on in *Settings*). You can
copy everything or export to `.md`; files go to the app data folder, under
`sessions/`.

While listening, ⌥ Space dictation is **disabled** on purpose: it would type
the text into the call itself.

### Things worth knowing

- **Starting capture can take a few seconds.** macOS builds a temporary audio
  device for the tap, and it is slower right after a previous session closed.
  The button shows *Starting…* meanwhile.
- **"Waiting for audio" is not an error.** The capture delivers nothing until
  something plays.
- **Headphones are recommended** if you enable the microphone track. On
  speakers your mic also hears the playback and the text would be duplicated.
  The app drops obvious echo automatically (*Suppress microphone echo*), but
  headphones solve it outright.
- **If the text comes out poor**, try the **Small** model in *Settings*.
  Transcription runs far faster than real time, so there is plenty of headroom.

### Per-system requirements

| System | Requirement |
| --- | --- |
| **macOS** | 14.6 or newer. On older versions the tab is disabled and dictation keeps working as normal. |
| **Windows** | None. |
| **Linux** | PulseAudio or PipeWire (with `pipewire-pulse`). A bare ALSA system does not expose output audio. |

> **Legal note:** recording or transcribing a conversation may require the
> participants' consent depending on where you are. That is on you.

---

## 7. Uninstall

- **macOS:** delete `/Applications/Local Flow.app` and
  `~/Library/Application Support/com.gabriel.local-flow` (models and config).
- **Windows:** uninstall from *Apps*; delete `%APPDATA%\com.gabriel.local-flow`.
- **Linux:** remove the package; delete `~/.local/share/com.gabriel.local-flow`.

---

## Architecture

```
UI (React)  ──invoke/events──►  Backend (Rust)
                                 ├── audio.rs    cpal capture + resample→16kHz
                                 ├── loopback.rs system audio capture
                                 ├── stream.rs   VAD chunking + live queue
                                 ├── live.rs     two-track session
                                 ├── session.rs  transcript and export
                                 ├── dsp.rs      resampling and anti-aliasing
                                 ├── whisper.rs  whisper.cpp transcription (local AI)
                                 ├── models.rs   model download/management
                                 ├── config.rs   JSON settings
                                 └── lib.rs      hotkey · tray · overlay · metrics
```

Third-party licenses: see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## Cutting a release (maintainers)

`.github/workflows/release.yml` builds on GitHub Actions for macOS
(Apple Silicon + Intel), Linux and Windows, and uploads the installers to the
release.

```sh
# 1. Bump the version in both places (they must match)
#    apps/desktop/src-tauri/tauri.conf.json  ->  "version"
#    apps/desktop/package.json               ->  "version"

# 2. Tag and push
git tag v0.1.0
git push origin v0.1.0
```

It can also be run manually from *Actions → Release → Run workflow* with the
tag as input. No secrets required: it uses the automatic `GITHUB_TOKEN`.
