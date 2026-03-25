# Chibby

Local-first, open-source CI/CD and deployment tool for solo developers and tiny teams.

## Overview

Chibby helps developers turn existing scripts into visual, repeatable pipelines that run locally or over SSH. Instead of learning complex CI platforms, you import your existing `deploy.sh`, `Makefile`, `justfile`, or package scripts and get clear logs, run history, retry, and rollback capabilities.

## Features

- **Pipeline Templates** — 20 built-in templates (9 full pipelines + 11 stage snippets) with variable substitution, import/export, and 3-layer resolution ([Templates docs](docs/features/templates.md))
- **GitHub Actions Import** — Import stages from existing `.github/workflows/` into your pipeline
- **CLI** — Standalone command-line interface for headless servers and scripting ([CLI docs](docs/features/cli-commands.md))
- **Script Import** — Detect and import existing scripts from your repo
- **Pipeline Generation** — Auto-generate pipelines from detected commands (heuristic + LLM-assisted)
- **Local Execution** — Run stages as local processes with live log streaming
- **SSH Execution** — Deploy over SSH with direct commands or Docker Compose
- **Environments & Secrets** — Per-environment config with OS keychain integration
- **Versioning** — Semver bumping across config files with automatic git tagging, configurable bump level (patch/minor/major)
- **Code Signing** — macOS notarization, Windows Authenticode, Linux GPG
- **Artifacts** — Consistent naming, SHA256 checksums, configurable retention
- **Tauri Updater** — Generate `latest.json`, sign update bundles, publish to hosting
- **Security Gates** — Secret scanning (gitleaks), CVE scanning, commit linting
- **Run History** — Full history with retry from failure and explicit rollback
- **Notifications** — Desktop OS notifications and webhooks (Slack, Discord, HTTP)
- **App Settings** — Configurable notification and retention defaults that apply across all projects
- **Cross-Platform** — Works on macOS, Linux, and Windows

## Why Chibby?

| | Chibby | GitHub Actions | GitLab CI | Jenkins | CircleCI |
| --- | --- | --- | --- | --- | --- |
| **Runs locally** | Yes — native, no containers needed | No (cloud) | No (cloud or self-hosted runner) | Self-hosted only | No (cloud) |
| **Zero config start** | Auto-detects scripts & generates pipelines | Manual YAML | Manual YAML | Manual Jenkinsfile | Manual YAML |
| **Internet required** | No — fully offline | Yes | Yes (or self-hosted) | No (self-hosted) | Yes |
| **Pricing** | Free & open-source | Free tier, paid minutes | Free tier, paid minutes | Free (self-hosted) | Free tier, paid credits |
| **Setup complexity** | Download and run | Repo + config + cloud | Repo + config + runners | Server + plugins + agents | Repo + config + cloud |
| **Secret management** | OS keychain (native) | Cloud secrets | Cloud variables | Credentials plugin | Cloud contexts |
| **Live logs** | Real-time in GUI & CLI | Delayed (cloud round-trip) | Delayed | Plugin-dependent | Delayed |
| **Run history & rollback** | Built-in with retry from failure | Re-run workflows | Retry jobs | Rebuild | Re-run |
| **SSH deploy** | First-class (direct + Docker Compose) | Via custom actions | Via scripts | Via plugins | Via orbs |
| **Best for** | Solo devs & tiny teams | Teams on GitHub | Teams on GitLab | Enterprise self-hosted | Teams wanting managed CI |

> **TL;DR** — Chibby is built for developers who want repeatable pipelines without cloud lock-in, YAML sprawl, or CI minutes. Import your existing scripts, run locally, deploy over SSH — done.

## Install

Download the latest release for your platform from
[GitHub Releases](https://github.com/Nyantahi/chibby/releases).

| Platform | Download |
| --- | --- |
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
2. **Choose a Source** — Auto-detect from build files, import from GitHub Actions, or start from a template
3. **Configure Stages** — Toggle, reorder, and edit the generated stages
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

```text
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
- **Project templates**: `.chibby/templates/` (in repo, shareable with team)
- **User templates**: `~/.chibby/templates/` (global, personal collection)
- **Run history & settings**: Platform app data directory
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

See [CONTRIBUTING.md](docs/community/CONTRIBUTING.md) for development setup, workflow, and code style guidelines.

## License

MIT
