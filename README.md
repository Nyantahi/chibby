# Chibby

Local-first, open-source CI/CD and deployment tool for solo developers and tiny teams.

## Overview

Chibby helps developers turn existing scripts into visual, repeatable pipelines that run locally or over SSH. Instead of learning complex CI platforms, you import your existing `deploy.sh`, `Makefile`, `justfile`, or package scripts and get clear logs, run history, retry, and rollback capabilities.

## Features

- **CLI** — Standalone command-line interface for headless servers and scripting ([CLI docs](docs/features/cli-commands.md))
- **Script Import** — Detect and import existing scripts from your repo
- **Pipeline Generation** — Auto-generate pipelines from detected commands (heuristic + LLM-assisted)
- **Local Execution** — Run stages as local processes with live log streaming
- **SSH Execution** — Deploy over SSH with direct commands or Docker Compose
- **Environments & Secrets** — Per-environment config with OS keychain integration
- **Versioning** — Semver bumping across config files with automatic git tagging
- **Code Signing** — macOS notarization, Windows Authenticode, Linux GPG
- **Artifacts** — Consistent naming, SHA256 checksums, configurable retention
- **Tauri Updater** — Generate `latest.json`, sign update bundles, publish to hosting
- **Security Gates** — Secret scanning (gitleaks), CVE scanning, commit linting
- **Run History** — Full history with retry from failure and explicit rollback
- **Notifications** — Desktop OS notifications and webhooks (Slack, Discord, HTTP)
- **Cross-Platform** — Works on macOS, Linux, and Windows

## Install

Download the latest release for your platform from
[GitHub Releases](https://github.com/Nyantahi/chibby/releases).

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Linux (Debian/Ubuntu) | `.deb` |
| Linux (Fedora/RHEL) | `.rpm` |
| Linux (any distro) | `.AppImage` |
| Windows | `.exe` (NSIS installer) |

See the [installation guide](docs/guides/installation.md) for detailed
instructions, SSH setup, and secrets configuration per platform.

### Build from source

```bash
git clone https://github.com/Nyantahi/chibby.git
cd chibby
npm install
npm run tauri:build
```

Requires Node.js 20+, Rust (stable), and
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

## Quick Start

1. **Add a Project** — Click "Add Project" and select a local repository
2. **Review Detected Scripts** — Chibby scans for `deploy.sh`, `Makefile`, `justfile`, `docker-compose.yml`, and package scripts
3. **Generate Pipeline** — Accept the suggested pipeline or customize stages
4. **Run** — Click "Run Pipeline" to execute all stages
5. **Monitor** — Watch live logs and stage status in real time

## Pipeline Configuration

Pipelines are stored as `.chibby/pipeline.toml` in each project:

```toml
name = "My Pipeline"

[[stages]]
name = "install"
commands = ["npm install"]
backend = "local"
fail_fast = true

[[stages]]
name = "test"
commands = ["npm test"]
backend = "local"
fail_fast = true

[[stages]]
name = "deploy"
commands = ["./deploy.sh"]
backend = "ssh"
fail_fast = true
```

See [examples/](examples/) for pipelines covering Node.js, Rust, Django, Docker
Compose, Tauri desktop apps, and static sites.

## Tech Stack

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri v2
- **Storage**: Local file system (pipelines in `.chibby/` per repo, run history in app data)
- **Secrets**: OS keychain (macOS Keychain, GNOME Keyring / KDE Wallet, Windows Credential Manager)

## Project Structure

```
chibby/
  frontend/           # React frontend
    components/       # UI components
    services/         # API calls to backend
    types/            # TypeScript interfaces
    styles/           # CSS styles
  src-tauri/          # Rust backend
    src/
      commands/       # Tauri command handlers
      engine/         # CI/CD engine
    build_checks/     # Platform-specific build validation
  examples/           # Example pipeline configurations
  docs/               # Documentation and guides
```

## Data Storage

- **Pipeline config**: `.chibby/pipeline.toml` (in repo, version controlled)
- **Environment config**: `.chibby/environments.toml` (in repo)
- **Secret references**: `.chibby/secrets.toml` (names only, no values)
- **Run history**: Platform app data directory
  - macOS: `~/Library/Application Support/Chibby/`
  - Linux: `~/.local/share/chibby/`
  - Windows: `%APPDATA%\Chibby\`

## Development

```bash
npm run tauri:dev      # Run development server
npm run type-check     # TypeScript type check
npm run lint           # ESLint
npm run format         # Prettier
npm test               # Vitest
npm run tauri:build    # Production build
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, workflow, and code style guidelines.

## License

MIT
