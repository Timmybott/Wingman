#!/usr/bin/env bash
# Feather installer for Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/Timmybott/Wingman/main/install.sh | bash
#
# Downloads the latest GitHub release and installs it:
#   - Debian/Ubuntu (apt available): installs the .deb (integrates with the
#     system package manager, uninstall via `sudo apt remove feather`)
#   - everything else: puts the AppImage into ~/.local/bin and adds a
#     desktop entry
# Updates after that are handled by the app's built-in updater.

set -euo pipefail

REPO="Timmybott/Wingman"
API="https://api.github.com/repos/$REPO/releases/latest"

say() { printf '\033[1;35m[feather]\033[0m %s\n' "$*"; }
fail() {
    printf '\033[1;31m[feather]\033[0m %s\n' "$*" >&2
    exit 1
}

case "$(uname -m)" in
x86_64 | amd64) ;;
*) fail "unsupported architecture: $(uname -m) (releases are currently built for x86_64)" ;;
esac

command -v curl >/dev/null 2>&1 || fail "curl is required"

say "Looking up the latest release…"
assets=$(curl -fsSL "$API" | grep '"browser_download_url"' | cut -d '"' -f 4) ||
    fail "could not query the latest release — does one exist yet?"

deb_url=$(printf '%s\n' "$assets" | grep -E '\.deb$' | head -n1 || true)
appimage_url=$(printf '%s\n' "$assets" | grep -E '\.AppImage$' | head -n1 || true)

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

if command -v apt-get >/dev/null 2>&1 && [ -n "$deb_url" ]; then
    say "Downloading $(basename "$deb_url")…"
    curl -fL --progress-bar -o "$tmpdir/feather.deb" "$deb_url"
    say "Installing via apt (this may ask for your sudo password)…"
    sudo apt-get install -y "$tmpdir/feather.deb"
    say "Done! Find Feather in your app menu or run: feather"
elif [ -n "$appimage_url" ]; then
    say "Downloading $(basename "$appimage_url")…"
    bin_dir="$HOME/.local/bin"
    mkdir -p "$bin_dir"
    curl -fL --progress-bar -o "$bin_dir/feather" "$appimage_url"
    chmod +x "$bin_dir/feather"

    app_dir="$HOME/.local/share/applications"
    mkdir -p "$app_dir"
    cat >"$app_dir/feather.desktop" <<DESKTOP
[Desktop Entry]
Name=Feather
Comment=Desktop client for Pterodactyl with one-click deploys
Exec=$bin_dir/feather
Terminal=false
Type=Application
Categories=Development;Utility;
DESKTOP

    say "Installed to $bin_dir/feather (AppImage) with a desktop entry."
    case ":$PATH:" in
    *":$bin_dir:"*) say "Done! Run: feather" ;;
    *) say "Done! Note: $bin_dir is not on your PATH — start Feather from your app menu." ;;
    esac
else
    fail "no suitable asset (.deb/.AppImage) found in the latest release"
fi
