# Changelog

All notable changes to Chibby will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial Tauri + React scaffold
- Project detection engine (Git, Node, Rust, Python, Go, Docker)
- Pipeline generation with step editing (heuristic + LLM-assisted)
- Run execution with real-time log streaming
- Environment variable and secrets management with OS keychain integration
- SSH execution backend (direct commands and Docker Compose)
- Preflight checks and recommendations panel
- Version bumping (semver), git tagging, and changelog generation
- Code signing: macOS notarization, Windows Authenticode, Linux GPG
- Artifact management with SHA256 checksums and configurable retention
- Tauri updater integration (latest.json generation, update signing, publishing)
- Security gates: secret scanning (gitleaks), CVE/dependency scanning, commit linting
- Desktop and webhook notifications (Slack, Discord, HTTP)
- Run history with retry from failed stage and explicit rollback
- Platform install packages: macOS DMG, Linux deb/rpm/AppImage, Windows NSIS
- Release CI workflow for automated cross-platform builds on tag push
- Example pipelines for Node.js, Rust, Django, Docker Compose, Tauri, static sites
- Platform-specific installation guide (macOS, Linux, Windows)
- Homebrew cask formula template
- Build-time validation modules for macOS, Linux, and Windows
- CI pipeline with lint, test, and build on all three platforms
- Pipeline templates system with 3-layer resolution (built-in, user, project)
- 19 built-in templates: 9 full pipelines + 10 stage snippets
- Template variable substitution with `{{variable}}` placeholders
- Template Browser component with search, category, type, and source filters
- Template Variable Dialog for filling in placeholders before applying
- Save As Template dialog for saving pipelines as reusable templates
- Template integration in Add Project wizard ("From Template" source option)
- Template integration in Pipeline Editor (dynamic stage templates from API)
- Tauri IPC commands for template CRUD, import, and export
- Frontend API service functions for all template operations
- CLI documentation (docs/features/cli-commands.md)
- Deploy step templates in Pipeline Editor (GitHub Release, Homebrew, Docker, SSH, S3, npm, Cargo, Tauri)
- Homebrew templates auto-detect repo URL and formula paths via `gh` CLI

### Changed

- README: added CLI feature entry with link to CLI docs
- README: removed internal roadmap section
- User guide: replaced roadmap references with changelog links
