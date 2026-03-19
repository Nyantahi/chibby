# Installation Guide

Chibby is available as a pre-built desktop app for macOS, Linux, and Windows.

## Download

Download the latest release for your platform from the
[GitHub Releases](https://github.com/okapian/chibby/releases) page.

| Platform | File | Notes |
|----------|------|-------|
| macOS (Apple Silicon) | `Chibby_x.x.x_aarch64.dmg` | Requires macOS 10.15+ |
| macOS (Intel) | `Chibby_x.x.x_x64.dmg` | Requires macOS 10.15+ |
| Linux (Debian/Ubuntu) | `chibby_x.x.x_amd64.deb` | |
| Linux (Fedora/RHEL) | `chibby-x.x.x-1.x86_64.rpm` | |
| Linux (any) | `chibby_x.x.x_amd64.AppImage` | No installation needed |
| Windows | `Chibby_x.x.x_x64-setup.exe` | NSIS installer |

## macOS

### DMG install

1. Download the `.dmg` file for your architecture
2. Open the DMG and drag **Chibby** to your **Applications** folder
3. On first launch, right-click the app and select **Open** to bypass Gatekeeper
   (or go to System Settings > Privacy & Security and click "Open Anyway")

### Homebrew (planned)

```bash
brew install --cask chibby
```

### Secrets and keychain

Chibby stores secrets (SSH keys, API tokens, signing credentials) in the macOS
Keychain. You may see a system prompt asking to allow Chibby access when you
first save a secret. Grant access to enable secure credential storage.

### SSH setup

Chibby uses your existing SSH configuration from `~/.ssh/config`. Ensure your
deploy targets are configured there:

```
Host myserver
  HostName 192.168.1.100
  User deploy
  IdentityFile ~/.ssh/id_ed25519
```

Chibby will use `ssh-agent` if running. If your key has a passphrase and you
are not running `ssh-agent`, add the key first:

```bash
ssh-add ~/.ssh/id_ed25519
```

## Linux

### Debian / Ubuntu (.deb)

```bash
sudo dpkg -i chibby_x.x.x_amd64.deb
sudo apt-get install -f  # install any missing dependencies
```

Required system libraries (installed automatically by the `.deb`):

- `libwebkit2gtk-4.1-0`
- `libappindicator3-1`
- `librsvg2-common`

### Fedora / RHEL (.rpm)

```bash
sudo rpm -i chibby-x.x.x-1.x86_64.rpm
```

Required packages:

- `webkit2gtk4.1`
- `libappindicator-gtk3`

### AppImage (any distro)

```bash
chmod +x chibby_x.x.x_amd64.AppImage
./chibby_x.x.x_amd64.AppImage
```

No installation needed. Move the file anywhere you like and run it directly.

For desktop integration (menu entry, file associations):

```bash
# Optional: use AppImageLauncher or manually create a .desktop file
mv chibby_x.x.x_amd64.AppImage ~/Applications/Chibby.AppImage
```

### Secrets and keyring

Chibby uses the system keyring via the Secret Service API (typically provided by
GNOME Keyring or KDE Wallet). If you are on a minimal desktop or a window
manager without a keyring daemon, install one:

```bash
# GNOME Keyring (works on most DEs)
sudo apt install gnome-keyring      # Debian/Ubuntu
sudo dnf install gnome-keyring      # Fedora
```

Ensure the keyring daemon is running in your session. Most full desktop
environments handle this automatically.

### SSH setup

Same as macOS — Chibby reads `~/.ssh/config` and uses `ssh-agent` if available.

```bash
# Start ssh-agent if not running
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519
```

## Windows

### NSIS installer

1. Download `Chibby_x.x.x_x64-setup.exe`
2. Run the installer — it supports per-user or system-wide installation
3. Launch Chibby from the Start menu

The installer bundles WebView2 and will download it automatically if not already
present on your system.

### Secrets and credential store

Chibby stores secrets in the Windows Credential Manager. You can view and manage
stored credentials through:

- Control Panel > User Accounts > Credential Manager
- Or search for "Credential Manager" in the Start menu

### SSH setup

Chibby uses your SSH configuration from `%USERPROFILE%\.ssh\config`. Windows 10+
includes OpenSSH by default.

If you use PuTTY-style keys (`.ppk`), convert them to OpenSSH format first:

```powershell
# Convert PuTTY key to OpenSSH format using puttygen
puttygen mykey.ppk -O private-openssh -o %USERPROFILE%\.ssh\id_ed25519
```

Start the SSH agent service if not running:

```powershell
Get-Service ssh-agent | Set-Service -StartupType Automatic
Start-Service ssh-agent
ssh-add $env:USERPROFILE\.ssh\id_ed25519
```

## Building from source

If no pre-built package is available for your platform, you can build Chibby
from source.

### Prerequisites

- [Node.js 20+](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Platform-specific Tauri dependencies:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Linux**: `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential curl wget libssl-dev libgtk-3-dev`
  - **Windows**: [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/), [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

### Build steps

```bash
git clone https://github.com/okapian/chibby.git
cd chibby
npm install
npm run tauri:build
```

Built artifacts appear in `src-tauri/target/release/bundle/`.

## Data locations

| Platform | App data | Config example |
|----------|----------|----------------|
| macOS | `~/Library/Application Support/Chibby/` | `~/Library/Application Support/Chibby/settings.json` |
| Linux | `~/.local/share/chibby/` | `~/.config/chibby/settings.json` |
| Windows | `%APPDATA%\Chibby\` | `%APPDATA%\Chibby\settings.json` |

Pipeline configurations are stored in each project's `.chibby/` directory and
are intended to be version-controlled alongside the project.

## Uninstalling

### macOS

Drag Chibby from Applications to Trash. To also remove data:

```bash
rm -rf ~/Library/Application\ Support/Chibby/
```

### Linux (deb)

```bash
sudo apt remove chibby
rm -rf ~/.local/share/chibby/
```

### Linux (rpm)

```bash
sudo rpm -e chibby
rm -rf ~/.local/share/chibby/
```

### Windows

Use "Add or Remove Programs" in Settings. To also remove data:

```powershell
Remove-Item -Recurse -Force "$env:APPDATA\Chibby\"
```
