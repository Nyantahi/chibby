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
- Templates documentation (docs/features/templates.md)
- Deploy step templates in Pipeline Editor (GitHub Release, Homebrew, Docker, SSH, S3, npm, Cargo, Tauri)
- Homebrew templates auto-detect repo URL and formula paths via `gh` CLI
- GitHub Actions import: parse `.github/workflows/` and convert steps to pipeline stages
- GitHub Actions import available in both Add Project wizard and Pipeline Editor
- "Apply Template" and "Use as Starting Point" buttons on Templates page navigate to Add Project wizard with template pre-selected
- Add Project wizard shows pre-selected template banner and skips source step when template is provided
- App Settings page with configurable notification and retention defaults
- Default notification settings (notify on success/failure) applied when no per-project config exists
- Default retention settings (artifact count, run history count) applied when no per-project config exists
- Post-run cleanup: automatic pruning of old artifacts and run history based on retention limits
- Configurable version bump level (`patch`, `minor`, `major`) in Version Bump & Tag template via `{{bump_level}}` variable
- Well-known template variable defaults with descriptions (bump_level, project_name)
- Dropdown selector for bump_level variable in Template Variable Dialog

### Changed

- README: added templates, GitHub Actions import, settings, and versioning features
- README: updated Quick Start to reflect new wizard flow with source selection
- README: added template storage paths to Data Storage section
- User guide: rewrote "Adding a Project" to cover 4-step wizard with template and GitHub Actions support
- User guide: added Pipeline Templates section with browsing, applying, and creating templates
- User guide: added Pipeline Editor enhancements (Import CI, Stage Templates, Save as Template)
- User guide: added App Settings section for notifications and retention defaults
- Version Bump & Tag template: bump level is now configurable instead of hardcoded to patch
