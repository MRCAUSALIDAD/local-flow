#!/usr/bin/env bash
# Local Flow installer for macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/MRCAUSALIDAD/local-flow/main/scripts/install.sh | bash
#
# Downloads the latest published installer for this machine and installs it.
# No build toolchain required.

set -euo pipefail

REPO="${LOCAL_FLOW_REPO:-MRCAUSALIDAD/local-flow}"
APP_NAME="Local Flow"

info() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m warn\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed."; }

need curl

TMPDIR_LF="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_LF"' EXIT

# ---------------------------------------------------------------- release info

api_url="https://api.github.com/repos/$REPO/releases/latest"
[ -n "${LOCAL_FLOW_TAG:-}" ] && api_url="https://api.github.com/repos/$REPO/releases/tags/$LOCAL_FLOW_TAG"

info "Looking up the latest release of $REPO..."
release_json="$TMPDIR_LF/release.json"
http_code="$(curl -sSL -w '%{http_code}' -H "Accept: application/vnd.github+json" \
  "$api_url" -o "$release_json" || echo 000)"

case "$http_code" in
  200) ;;
  404) die "No published release found for $REPO. Build from source instead (see the README)." ;;
  403) die "GitHub API rate limit reached. Try again later, or download the installer manually from https://github.com/$REPO/releases/latest" ;;
  000) die "Could not reach the GitHub API. Are you online?" ;;
  *) die "GitHub API returned HTTP $http_code." ;;
esac

# Pick the first asset download URL whose file name matches the given regex.
asset_url() {
  grep -o '"browser_download_url": *"[^"]*"' "$release_json" \
    | sed 's/.*": *"//; s/"$//' \
    | grep -E "$1" \
    | head -n1
}

download() {
  info "Downloading $(basename "$1")"
  curl -fL --progress-bar "$1" -o "$2"
}

os="$(uname -s)"
arch="$(uname -m)"

# ---------------------------------------------------------------------- macOS

install_macos() {
  case "$arch" in
    arm64 | aarch64) pattern='aarch64\.dmg$' ;;
    x86_64) pattern='x64\.dmg$' ;;
    *) die "Unsupported macOS architecture: $arch" ;;
  esac

  url="$(asset_url "$pattern")"
  [ -n "$url" ] || die "No .dmg found in the latest release for $arch."

  dmg="$TMPDIR_LF/local-flow.dmg"
  download "$url" "$dmg"

  info "Mounting the disk image..."
  mount_point="$TMPDIR_LF/mnt"
  mkdir -p "$mount_point"
  hdiutil attach "$dmg" -nobrowse -quiet -mountpoint "$mount_point"
  # shellcheck disable=SC2064
  trap "hdiutil detach '$mount_point' -quiet >/dev/null 2>&1 || true; rm -rf '$TMPDIR_LF'" EXIT

  src="$mount_point/$APP_NAME.app"
  [ -d "$src" ] || die "The disk image does not contain $APP_NAME.app"

  dest="/Applications/$APP_NAME.app"
  sudo_cmd=""
  if [ ! -w /Applications ]; then
    warn "/Applications needs administrator rights; you may be asked for your password."
    sudo_cmd="sudo"
  fi

  if [ -d "$dest" ]; then
    info "Removing the previous version..."
    $sudo_cmd rm -rf "$dest"
  fi

  info "Installing to $dest"
  $sudo_cmd cp -R "$src" "$dest"

  # The build is not notarized, so strip the download quarantine flag; otherwise
  # macOS refuses to open it with a "damaged app" dialog.
  $sudo_cmd xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true

  hdiutil detach "$mount_point" -quiet >/dev/null 2>&1 || true

  info "Done. Launching $APP_NAME..."
  open "$dest" || true
  cat <<EOF

Next steps:
  1. Settings -> Voice model -> Download a model (one time, then fully offline).
  2. Grant Microphone access when prompted.
  3. Grant Accessibility access (System Settings -> Privacy & Security ->
     Accessibility -> enable $APP_NAME) so it can type into other apps.
EOF
}

# ---------------------------------------------------------------------- Linux

install_linux() {
  case "$arch" in
    x86_64 | amd64) deb_pat='amd64\.deb$'; app_pat='(amd64|x86_64)\.AppImage$' ;;
    aarch64 | arm64) deb_pat='arm64\.deb$'; app_pat='(arm64|aarch64)\.AppImage$' ;;
    *) die "Unsupported Linux architecture: $arch" ;;
  esac

  sudo_cmd=""
  if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 && sudo_cmd="sudo"
  fi

  # Prefer a real package on Debian/Ubuntu: it wires up the menu entry and the
  # shared-library deps for us.
  if command -v apt-get >/dev/null 2>&1; then
    url="$(asset_url "$deb_pat")"
    if [ -n "$url" ]; then
      deb="$TMPDIR_LF/local-flow.deb"
      download "$url" "$deb"
      info "Installing the .deb package (may ask for your password)..."
      $sudo_cmd apt-get install -y "$deb"
      info "Done. Launch 'Local Flow' from your applications menu."
      return
    fi
    warn "No .deb in the release; falling back to the AppImage."
  fi

  url="$(asset_url "$app_pat")"
  [ -n "$url" ] || die "No installable Linux asset found for $arch."

  bindir="${XDG_BIN_HOME:-$HOME/.local/bin}"
  mkdir -p "$bindir"
  target="$bindir/local-flow"
  download "$url" "$target"
  chmod +x "$target"

  # Desktop entry so it shows up in the launcher like a normal app.
  desktop_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  mkdir -p "$desktop_dir"
  cat >"$desktop_dir/local-flow.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=Offline voice dictation
Exec=$target
Terminal=false
Categories=Utility;AudioVideo;
EOF
  command -v update-desktop-database >/dev/null 2>&1 \
    && update-desktop-database "$desktop_dir" >/dev/null 2>&1 || true

  info "Installed to $target"
  case ":$PATH:" in
    *":$bindir:"*) ;;
    *) warn "$bindir is not on your PATH. Add it, or run the binary directly." ;;
  esac
  cat <<EOF

Notes:
  - The AppImage needs FUSE. On Ubuntu: sudo apt install libfuse2
  - Wayland limits global hotkeys and keystroke injection. If typing into other
    apps fails, use the clipboard (on by default) and paste manually.
EOF
}

case "$os" in
  Darwin) install_macos ;;
  Linux) install_linux ;;
  *) die "Unsupported OS: $os. On Windows use scripts/install.ps1" ;;
esac
