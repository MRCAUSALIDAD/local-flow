# Local Flow

**🌐 Idioma:** **Español** · [English](README.en.md)

Dictado por voz **100% local y privado** para macOS, Windows y Linux.
Mantén pulsada una tecla, habla, suéltala — el texto transcrito se escribe
directamente en la app donde estés (y se copia al portapapeles).

## 🔒 Privacidad: todo corre en local

- **El audio nunca sale de tu ordenador.** Se captura, se transcribe y se
  descarta en tu propia máquina.
- **Sin nube, sin servidores, sin telemetría.** No se envía nada a ningún
  servicio externo ni de terceros.
- La transcripción usa **Whisper (whisper.cpp)**, un modelo de IA que corre
  **localmente** sobre tu CPU.
- **Única conexión a internet:** descargar una vez el modelo de voz desde
  Hugging Face. Después funciona totalmente **offline**.

---

## ⬇️ Instalación

No hace falta compilar nada ni instalar Rust: descarga el instalador ya hecho.

### Instalador de una línea

**macOS y Linux**

```sh
curl -fsSL https://raw.githubusercontent.com/MRCAUSALIDAD/local-flow/main/scripts/install.sh | bash
```

**Windows** (PowerShell)

```powershell
irm https://raw.githubusercontent.com/MRCAUSALIDAD/local-flow/main/scripts/install.ps1 | iex
```

El script detecta tu sistema y arquitectura, descarga el instalador de la
última release y lo instala (en macOS además quita la cuarentena de Gatekeeper).

### Descarga manual

Desde [**Releases**](https://github.com/MRCAUSALIDAD/local-flow/releases/latest):

| Sistema | Archivo | Cómo se instala |
| --- | --- | --- |
| macOS (Apple Silicon) | `..._aarch64.dmg` | Abrir y arrastrar a *Aplicaciones* |
| macOS (Intel) | `..._x64.dmg` | Abrir y arrastrar a *Aplicaciones* |
| Windows | `..._x64-setup.exe` | Ejecutar el instalador |
| Debian/Ubuntu | `..._amd64.deb` | `sudo apt install ./<archivo>.deb` |
| Otras distros | `..._amd64.AppImage` | `chmod +x` y ejecutar |

> Las builds **no están firmadas** (no hay certificado de desarrollador).
> En macOS: clic derecho → **Abrir**, o
> `xattr -dr com.apple.quarantine "/Applications/Local Flow.app"`.
> En Windows: SmartScreen → **Más información → Ejecutar de todos modos**.

Tras instalar, sigue con el [paso 3: descargar el modelo de voz](#3-descargar-el-modelo-de-voz).

---

## 1. Requisitos previos (solo para compilar desde el código)

Necesitas tres cosas base en cualquier sistema:

- **Node.js 18+** y npm — https://nodejs.org
- **Rust** (toolchain estable) — https://rustup.rs
- **Toolchain de C/C++ + CMake** (whisper.cpp se compila en tu máquina).

### macOS

```sh
xcode-select --install          # herramientas de compilación C/C++
brew install cmake node rustup-init && rustup-init
```

### Windows

- Instala **Microsoft C++ Build Tools** (workload "Desktop development with C++").
- Instala **CMake** — https://cmake.org/download (marca "Add to PATH").
- Instala **Node.js** y **Rust** (rustup).
- **WebView2**: ya viene en Windows 11 (en Windows 10 se instala solo con la app).

### Linux (Debian/Ubuntu)

```sh
sudo apt update
sudo apt install -y build-essential cmake curl file \
  libasound2-dev libxdo-dev \
  libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev \
  libgtk-3-dev libayatana-appindicator3-dev
# Node y Rust: instala desde nodejs.org y rustup.rs
```

> `libasound2-dev` = micrófono (cpal) · `libxdo-dev` = escribir en otras apps
> (enigo) · el resto = Tauri/WebKit.

---

## 2. Instalar y ejecutar

```sh
# 1. Clona el repositorio
git clone <url-del-repo> local-flow
cd local-flow/apps/desktop

# 2. Instala dependencias de frontend
npm install

# 3a. Modo desarrollo (recarga en vivo)
npm run tauri dev
```

La primera compilación de Rust tarda varios minutos (compila whisper.cpp);
las siguientes son rápidas por caché.

### Crear la app instalable

```sh
npm run tauri build
```

Resultados en `src-tauri/target/release/bundle/`:

- **macOS:** `macos/Local Flow.app` (arrástrala a `/Applications`) y
  `dmg/Local Flow_*.dmg`
- **Windows:** `msi/` o `nsis/` (instalador `.exe`)
- **Linux:** `deb/`, `rpm/` o `appimage/`

---

## 3. Descargar el modelo de voz

Al abrir la app por primera vez:

1. Ve a **Settings → Voice model**.
2. Pulsa **Download** en un modelo:
   - **Tiny** (~75 MB) — el más rápido, menor precisión.
   - **Base** (~142 MB) — recomendado, equilibrado.
   - **Small** (~466 MB) — más lento, mejor precisión.
3. Se descarga una vez, se activa solo y el estado pasa a **Model ready**.

A partir de aquí funciona **sin internet**.

---

## 4. Gestionar permisos

### macOS

Dos permisos, ambos gestionados en *Ajustes del Sistema → Privacidad y seguridad*:

1. **Micrófono** — se pide al primer dictado. Acéptalo.
2. **Accesibilidad** — necesario para que escriba en **otras** apps.
   - En la app aparece un aviso con botón para abrir el panel, o ve a
     *Privacidad y seguridad → Accesibilidad* y **activa Local Flow**.

> **Importante (builds sin firmar):** cada vez que **recompilas** la app, macOS
> puede invalidar el permiso de Accesibilidad aunque la casilla siga marcada.
> Si deja de escribir en otras apps: quita Local Flow de la lista con **–**,
> vuelve a añadirla con **+**, y reinicia la app.

> **Gatekeeper:** en el primer arranque de una build sin firmar, haz
> clic derecho en la app → **Abrir**, o ejecuta:
> `xattr -dr com.apple.quarantine "/Applications/Local Flow.app"`

### Windows

- No requiere permisos especiales para escribir en otras apps.
- Windows Defender SmartScreen puede avisar en una app sin firmar:
  **Más información → Ejecutar de todos modos**.

### Linux

- Sesiones **X11**: funciona directamente.
- Sesiones **Wayland**: la inyección de teclado (enigo) y los atajos globales
  pueden estar limitados por el compositor; si falla escribir en otras apps,
  usa el portapapeles (activado por defecto) y pega manualmente.

---

## 5. Uso

- **Mantén `⌥ Space`** (Alt+Espacio) en cualquier app → habla → suelta.
- El texto se **escribe donde tengas el cursor** y se copia al portapapeles.
- **Punto indicador** flotante (siempre encima): verde = activo, cian pulsante =
  dictando, ámbar = transcribiendo.
- **Métricas:** pasa el ratón por encima del punto para ver CPU, RAM, duración
  de audio, tiempo de transcripción y velocidad. En *Settings* puedes dejarlas
  **fijas** en una esquina.
- Cerrar la ventana la manda a la **bandeja del sistema**; reábrela desde ahí.

---

## 6. Desinstalar

- **macOS:** borra `/Applications/Local Flow.app` y
  `~/Library/Application Support/com.gabriel.local-flow` (modelos y config).
- **Windows:** desinstala desde *Aplicaciones*; borra `%APPDATA%\com.gabriel.local-flow`.
- **Linux:** elimina el paquete; borra `~/.local/share/com.gabriel.local-flow`.

---

## Arquitectura

```
UI (React)  ──invoke/eventos──►  Backend (Rust)
                                  ├── audio.rs    captura cpal + resample→16kHz
                                  ├── whisper.rs  transcripción whisper.cpp (IA local)
                                  ├── models.rs   descarga/gestión de modelos
                                  ├── config.rs   ajustes en JSON
                                  └── lib.rs      hotkey · tray · overlay · métricas
```

Licencias de terceros: ver [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

---

## Publicar una versión (mantenedores)

`.github/workflows/release.yml` compila en GitHub Actions para macOS
(Apple Silicon + Intel), Linux y Windows, y sube los instaladores a la release.

```sh
# 1. Sube la versión en los dos sitios (deben coincidir)
#    apps/desktop/src-tauri/tauri.conf.json  ->  "version"
#    apps/desktop/package.json               ->  "version"

# 2. Etiqueta y empuja
git tag v0.1.0
git push origin v0.1.0
```

También se puede lanzar a mano desde *Actions → Release → Run workflow*
indicando el tag. No hace falta ningún secreto: usa el `GITHUB_TOKEN`
automático.
