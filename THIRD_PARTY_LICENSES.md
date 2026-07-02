# Third-Party Licenses

Local Flow bundles and links against the third-party software listed below.
Each dependency remains under its own license. This document lists the
**direct** dependencies; each of these in turn pulls in transitive
dependencies under permissive licenses (mostly MIT / Apache-2.0 / BSD).

To regenerate a complete, transitive report:

- Rust: `cargo install cargo-about && cargo about generate about.hbs`
  (or `cargo license`) from `apps/desktop/src-tauri`.
- Node: `npx license-checker --production` from `apps/desktop`.

---

## Rust crates (`apps/desktop/src-tauri/Cargo.toml`)

| Crate | Version | License |
| --- | --- | --- |
| tauri | 2 | Apache-2.0 OR MIT |
| tauri-build | 2 | Apache-2.0 OR MIT |
| tauri-plugin-opener | 2 | Apache-2.0 OR MIT |
| tauri-plugin-global-shortcut | 2 | Apache-2.0 OR MIT |
| tauri-plugin-clipboard-manager | 2 | Apache-2.0 OR MIT |
| tauri-plugin-dialog | 2 | Apache-2.0 OR MIT |
| serde | 1 | Apache-2.0 OR MIT |
| serde_json | 1 | Apache-2.0 OR MIT |
| anyhow | 1 | Apache-2.0 OR MIT |
| dirs | 5 | Apache-2.0 OR MIT |
| cpal | 0.15 | Apache-2.0 |
| whisper-rs | 0.14 | MIT |
| enigo | 0.3 | MIT |
| sysinfo | 0.32 | MIT |
| reqwest | 0.12 | Apache-2.0 OR MIT |
| macos-accessibility-client | 0.0.1 | Apache-2.0 OR MIT |

## Node packages (`apps/desktop/package.json`)

| Package | Version | License |
| --- | --- | --- |
| @tauri-apps/api | ^2 | Apache-2.0 OR MIT |
| @tauri-apps/plugin-clipboard-manager | ^2.3.2 | Apache-2.0 OR MIT |
| @tauri-apps/plugin-dialog | ^2.7.1 | Apache-2.0 OR MIT |
| @tauri-apps/plugin-opener | ^2 | Apache-2.0 OR MIT |
| react | ^19.1.0 | MIT |
| react-dom | ^19.1.0 | MIT |
| @tauri-apps/cli | ^2 | Apache-2.0 OR MIT |
| @types/react | ^19.1.8 | MIT |
| @types/react-dom | ^19.1.6 | MIT |
| @vitejs/plugin-react | ^4.6.0 | MIT |
| typescript | ~5.8.3 | Apache-2.0 |
| vite | ^7.0.4 | MIT |

## Native / model components

| Component | License | Notes |
| --- | --- | --- |
| whisper.cpp | MIT | Vendored through `whisper-rs`; performs local speech-to-text. |
| ggml | MIT | Tensor library underlying whisper.cpp. |
| Whisper models (`ggml-tiny/base/small`) | MIT | OpenAI Whisper weights, downloaded at runtime from Hugging Face. |

---

## License texts

### MIT License

```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Apache License 2.0

Full text: https://www.apache.org/licenses/LICENSE-2.0

Dependencies marked "Apache-2.0 OR MIT" may be used under either license at
your option; Local Flow relies on them under their permissive terms.
