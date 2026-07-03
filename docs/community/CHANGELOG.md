# Changelog

All notable changes to Chibby will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-03

### Added — Concurrent multi-project runs

- **Run pipelines for multiple projects at the same time** — starting a run is now fire-and-forget: you can kick off another project's pipeline, or navigate away, while the first keeps running. Backend execution was already isolated per repo; the blocker was a single-run frontend, now removed.
- **Per-run live state via a run store** — new `frontend/services/runStore.ts` (a lightweight module store consumed via `useSyncExternalStore`) holds each project's live stage/command status and log tail, keyed by repo path, and survives route navigation. A single global `pipeline:log` listener feeds it.
- **Tagged log events** — `pipeline:log` now carries `run_id` + `repo_path`, so concurrent runs are demultiplexed to the right project instead of interleaving into one stream.
- **At-a-glance running status** — the Projects list shows a live **"Running"** status (replacing the stale last-run summary) with a spinner while a pipeline is in flight, plus a sidebar list of every in-progress run. Each card settles back to Success / Failed / Cancelled on completion.
- **Same-project double-run guard** — the Run button is disabled while that project is already running. Cross-project concurrency is unaffected.

### Changed

- **Atomic `projects.json` writes** — the per-project run-summary update now serializes its read-modify-write behind a process lock (`persistence::mutate_projects`), so simultaneous run completions can no longer clobber each other's `last_run_status`.

### Added

- **Docs link check CI** — new `.github/workflows/docs.yml` runs lychee in offline mode on every markdown change. It verifies every relative link and image reference across `README.md` + `docs/**` resolves to a real file and fails the build otherwise. Offline mode skips external URLs, so the check stays deterministic (no network flakiness / false failures).

### Fixed

- **Project Detail header & tabs overlapped the sidebar on narrow windows** — neither the header action row (pipeline/target selectors + Run/Delete buttons) nor the tab strip (Pipeline / History / Environments / Release / Quality) could shrink, so once the window narrowed they overflowed `.page-main` and the right-hand sidebar painted over the "CI" selector and the Release/Quality tabs. The header actions now wrap, and the tab strip scrolls horizontally instead of overflowing.
- **CI workflow never triggered** — `.github/workflows/ci.yml` assumed a nested `chibby/` subdirectory (`paths: 'chibby/**'`, `working-directory: chibby`, `chibby/`-prefixed action inputs) that no longer matches the repo layout — the repo root *is* `chibby/`. The path filter never matched, so the lint / test / build jobs were silently skipped. Stripped the `chibby/` prefix throughout to match the root-path convention already used by `security.yml`.
- **Broken documentation links** — fixed five dangling relative links surfaced by the new link check: `README.md`'s examples link (`examples/` → `docs/examples/`), and four in `docs/guides/user-guide.md` (`../README.md` → `../../README.md`, a stale link into the gitignored `private/audits/` redirected to `../examples/`, and two changelog links → `../community/CHANGELOG.md`).

## [0.1.36] - 2026-05-21

### Added — Security gates (Phase 2 of the gates epic)

- **Four new gates** alongside the existing secret / dependency / commit-lint trio:
  - **`sast`** — wraps `semgrep --config=auto`. Catches SQLi, XSS, command injection, insecure crypto, dangerous subprocess use.
  - **`container_scan`** — wraps `trivy image`. Scans image refs from `gates.toml`'s `container_images` list; falls back to Dockerfiles auto-detected in the repo (top-level + 1 dir deep).
  - **`iac_scan`** — wraps `trivy config`. Catches Dockerfile / docker-compose / Kubernetes / Terraform / CloudFormation misconfigurations.
  - **`license_check`** — wraps `cargo-license` + `license-checker`. Flags GPL/AGPL by default; configurable via `license_denylist` + `license_allowlist`.
- **Graceful scanner-missing handling** — every gate detects its CLI at runtime and returns a non-failing `"(missing)"` result with the install command when the tool isn't installed. No more crashes on "semgrep not found."
- **Auto-appended pipeline stages** — when `.chibby/gates.toml` exists, `chibby pipeline generate` (and the GUI's Regenerate button) append one `security-<gate>` stage per enabled gate to the produced `pipeline.toml`. Off-mode gates are skipped.
- **Default `gates.toml` on project add** — `auto_bootstrap_for_project` now seeds a sensible default (`warn` mode everywhere, baseline mode on, test fixtures allowlisted) so the Quality tab is populated from day one. Won't overwrite an existing file.
- **New CLI subcommands** — `chibby scan sast`, `chibby scan container`, `chibby scan iac`, `chibby scan license`. Same shape as the existing `secrets`/`deps`/`commits`; non-zero exit on blocking findings.
- **Recommendations panel entries** — `.chibby/gates.toml` and `.github/workflows/security.yml` are now flagged when missing (High priority, Security category).
- **`GatesConfig` schema extended** with new mode fields (`sast`, `container_scan`, `iac_scan`, `license_check`), severity thresholds (`sast_severity_threshold`, `container_severity_threshold`, `iac_severity_threshold`), allowlists (`sast_allowlist`, `license_allowlist`), `container_images`, and `license_denylist`.
- **`GatesResult` schema extended** with `sast`, `container_scan`, `iac_scan`, `license_check` optional fields.
- **`GatesCard` (Quality tab)** — surfaces all seven gate-mode selectors, three severity threshold inputs, the `container_images` textarea, and four new Run buttons.
- **New docs** — [`docs/features/security-gates.md`](../features/security-gates.md) covers all seven gates, config keys, scanner install hints, and pipeline auto-append behaviour. Cross-linked from cli-commands.md and user-guide.md.

### Fixed

- **`chibby projects list/add/remove/info` were stubs** that printed hardcoded demo data (`my-app/website/api/mobile-app`). They now read/write `<data_dir>/projects.json` via the same `persistence` layer the GUI uses. `add` validates the path exists; `remove` resolves by id/name/path/abs-path; `info` defaults to the project whose path matches CWD.
- **`chibby scan secrets/deps/commits` were stubs** that slept and printed "no findings". They now call `gates::run_secret_scan/run_dependency_audit/run_commit_lint` and exit non-zero on blocking findings.
- **`bootstrap.rs` test imports** — re-added `EnvironmentsConfig`/`SecretsConfig` to the test module after the earlier prod-side import cleanup.

### Added

- **Project Detail is now 5 tabs** — Pipeline, History, **Environments**, **Release**, **Quality**. Every Tauri command in the backend now has a UI entry point; "Project Settings" is gone in favour of the three focused tabs.
- **Environments tab** — promotes the existing `EnvironmentEditor` and `SecretsManager` out of the collapsed Settings section and adds:
  - **Bootstrap button** — opens a modal that runs `scan_bootstrap`, shows every detected name with classification + provenance, and applies in Safe or Merge mode.
  - **Import button** — modal driver for `dotenv`, `vercel`, `railway`, and `fly` importers with a vendor-CLI presence check and report summary.
  - **Export .env button** — save-dialog-driven `export_dotenv` for a chosen environment.
  - **Inline leak warnings** in `EnvironmentEditor` — banner listing every `EnvLeakHit` from `scan_environments_for_leaks` (redacted previews).
  - **Committed / Local / Layered toggle** in `EnvironmentEditor` — switches between editing `environments.toml`, `environments.local.toml`, and the read-only merged view.
  - **Per-secret audit modal** in `SecretsManager` — clock icon on each row opens `get_secret_audit` (last set/deleted, counts, provenance).
- **Auto-bootstrap on Add Project** — the wizard finish step now calls `auto_bootstrap_for_project`. In `confirm` mode the review modal appears before navigation; in `silent` mode the apply happens and a toast confirms; in `off` mode nothing runs.
- **Release tab** — surfaces all of Phase 5 with one card each:
  - `VersionCard` — `detect_versions`, semver bump (patch/minor/major) with optional git tag, `generate_changelog` with copy-to-clipboard.
  - `ArtifactsCard` — artifact config form, `collect_artifacts`, manifest list with "Reveal output dir", inline Signing sub-section + per-artifact `sign_artifact`.
  - `UpdaterCard` — updater config, key management (generate / rotate / delete / import), `updater_preflight`, `generate_latest_json`, dry-run / live `publish_update`.
  - `NotifyCard` — notification targets editor and `send_test_notification`.
- **Quality tab** — `GatesCard` (config + run all gates / individual scans / create baseline), `CleanupCard` (config + dry-run / live `run_cleanup`), `DeploymentHistoryCard` (per-environment `get_deployment_history` table).
- **Crash log page** — new `/crashes` route reading `get_crash_log` with Reveal-in-Finder and Clear buttons; linked from Settings → About.
- **Quick Links sidebar card** on Project Detail — reveal `.chibby/`, reveal app data dir, open repo folder.
- **Plugin-shell wiring** — `services/openExternal.ts` wraps `@tauri-apps/plugin-shell` for OS-native opens. Recommendations panel doc links, Run Detail's new "Reveal" button (opens `<data_dir>/runs/<run_id>/`), Settings' "Open data directory", and every "Reveal in Finder" affordance route through this helper.
- **In-app toaster** — `services/notify.ts` + `Toaster` component mounted in `Layout`. Replaces the previous mix of `alert()` and silent failures with non-blocking toasts.
- **Bootstrap mode setting** in Settings → About — `confirm` / `silent` / `off` picker, persisted to `AppSettings.bootstrap_mode`.

### Fixed

- **sha2 0.11 hash formatting** — `Sha256::finalize()` now returns a `hybrid_array::Array` that no longer implements `LowerHex`; replaced `format!("{:x}", hash)` with explicit per-byte hex in `engine/artifacts.rs`.
- **Unused imports in `engine/bootstrap.rs`** — dropped `EnvironmentsConfig` and `SecretsConfig` from the model imports.
- **Security Scans workflow failures on `main`** — both jobs were red after the PR #60 merge interleaved with the Dependabot stack: `npm ci` rejected `package-lock.json` (esbuild 0.28.0 in lock vs 0.27.7 resolved) and `cargo audit` choked on stale `phf 0.8.0`/`0.10.1` entries pulling the yanked `proc-macro-hack`. Lockfile regenerated, Cargo.lock pruned, and the cargo-audit step pinned to a known-good version with `Swatinem/rust-cache`.

### Dependencies

- `tauri` Rust crate `2.9.1` → `2.11.2` (fixes the `tauri (v2.10.3) : @tauri-apps/api (v2.11.0)` mismatch that was blocking `npx tauri build`).
- `tauri-build` `2.5.1` → `2.6.2`.
- `@tauri-apps/cli` `^2.9.1` → `^2.11.2` (was reported outdated).
- `vite` `^8.0.2` → `^8.0.13` then `^8.0.14`. Closes `GHSA-4w7w-66w2-5vf9` (high-severity advisory flagged by `chibby scan deps`).
- `eslint-plugin-react-hooks` `^7.0.1` → `^7.1.1` (declares ESLint 10 in peer range — fixes `ERESOLVE` on install).
- Plus the routine Dependabot bumps for `eslint`, `prettier`, `typescript-eslint`, `esbuild`, `lucide-react`, `react-dom`.

### Changed

- `AppSettings` TypeScript interface now includes `bootstrap_mode: BootstrapMode` to match the Rust struct.
- `ProjectDetail` legacy router-state `tab: 'settings'` deep-links are silently rewritten to `tab: 'environments'` to keep old links working.

## [0.1.34] - 2026-05-08

### Fixed

- **Phantom duplicate config files on macOS/Windows** — `Makefile` (or any case-only variant) is no longer reported as a conflict against itself, and the "Detected Files" sidebar lists it once. `Path::exists()` is case-insensitive on APFS/NTFS, so probing each candidate name separately double-counted a single on-disk file; detection now intersects pattern names against actual directory entries.
- Same fix applied to subdirectory script detection (`frontend/`, `backend/`, …) so a single `Makefile` in a subdir is not duplicated.

## [0.1.32] - 2026-05-01

### Fixed

- **Pipeline generation ignores custom Tauri/Rust subdirectory layout** — projects whose `Cargo.toml` and `tauri.conf.json` live in a non-standard folder (e.g. `backend/`) no longer lose their cargo-build, cargo-test, and tauri-build stages when creating a project or hitting Regenerate in the pipeline editor.
- Root-level Cargo/Tauri detection now uses exact filename matching (`has_file`) so a `backend/Cargo.toml` cannot trigger the standard `src-tauri/` stage logic.

### Added

- `ProjectFolder` struct gains `has_rust` and `has_tauri` fields; `detect_project_folders` now recognises Rust (`Cargo.toml`) subdirectories alongside Node.js and Python ones.
- Subdir Rust stage generation in the project-folders loop: emits `cargo build --release --manifest-path <subdir>/Cargo.toml`, `cargo test --manifest-path <subdir>/Cargo.toml`, and `npx tauri build -c <subdir>/tauri.conf.json` when a subdirectory contains a custom Tauri config.

## [0.1.29] - 2026-04-11

### Added

- **Deployment configuration step** in project creation wizard for selecting deployment method
- **Auto-create environments.toml** with sensible defaults when deploy pipeline is generated
  - SSH-based deploys (Docker Compose SSH, Docker Registry, rsync) create production + staging
  - PaaS deploys (Fly.io, Render, Railway, Vercel, Netlify, S3) create production only
- **Fullstack project detection** for monorepos with frontend/backend/admin folders
- **Multi-folder pipeline generation** with per-folder stages (install, test, build)
- GitHub Actions deploy workflow parsing integrated into pipeline generation
- Scripts directory detection (`scripts/`) for shell script discovery
- Docker Compose variant detection (`docker-compose.prod.yml`, `docker-compose.staging.yml`, etc.)
- Python test icons (pytest, test directories) in detected files list
- Tooltip on detected files showing full file path

### Fixed

- Grey out disabled pipeline/target dropdowns instead of hiding them
- Always show pipeline and target dropdowns for consistent UI layout
- Only generate root npm stages when root `package.json` exists (fixes duplicate stages)
- Only generate root Python stages when root `requirements.txt` exists
- Use `npm install` instead of `npm ci` for broader compatibility
- Git branch text overflow with ellipsis for long branch names

### Changed

- Widen page max-width from 960px to 1600px for better screen utilization

## [0.1.28] - 2026-04-04

### Security

- **Critical**: Prevent path traversal in pipeline save/load via name validation
- **Critical**: Sanitize log lines in agent context to block prompt injection
- **Critical**: Redact secret patterns (passwords, API keys, tokens) before sending logs to AI APIs
- **Critical**: Validate environment variable names in SSH export commands
- **Critical**: Fix keychain key collision using percent-encoded delimiters
- **Critical**: Validate SSH host to prevent option injection
- **High**: Add input length limits to agent chat (8 KB) and pipeline generation (16 KB)
- **High**: Add rate limiting to AI API calls (15 requests/minute token bucket)
- **High**: Set restrictive permissions (chmod 700) on app data directory on Unix
- **Medium**: Validate environment variable names in frontend EnvironmentEditor
- **Medium**: Add audit logging for sensitive operations (secrets, API keys, environments)
- **Medium**: Validate agent-generated pipeline save paths stay within project directory

### Added

- `security:audit` npm script for running `cargo audit` on Rust dependencies
- Audit log file (`<data_dir>/audit.log`) for tracking sensitive operations

### Fixed

- Version displayed in sidebar now fetched dynamically from backend instead of hardcoded
- Synced version across all config files (package.json, Cargo.toml, tauri.conf.json, Homebrew formulas)

## [0.1.26] - 2026-04-03

### Added

- 15 cloud provider deployment stage templates:
  - **Fly.io**: Container deploy, Static site deploy
  - **Render**: CLI deploy, Git push deploy
  - **AWS**: ECS (Fargate/EC2) deploy, EC2 SSH deploy
  - **Google Cloud**: Cloud Run deploy, GCE SSH deploy
  - **Azure**: Web App / App Service deploy
  - **Railway**: CLI deploy
  - **DigitalOcean**: App Platform deploy, Droplet SSH deploy
  - **Hetzner**: Cloud SSH deploy
  - **Akamai/Linode**: Linode SSH deploy, EdgeWorkers deploy
- Health check configurations for deployment verification (retries, delays)
- Preflight tool validation for cloud CLIs (flyctl, gcloud, az, railway, doctl, akamai)

## [0.1.8] - 2026-03-26

### Added

- Failed-run retry banner in UI
- Split Homebrew template into Tap Publish and Core PR templates
- App-level notification and retention defaults wired into pipeline runs
- Post-run housekeeping and backlog planning

### Fixed

- Resolve peer dependency conflicts preventing `npm ci` (@eslint/js, @vitest/coverage-v8, typescript)
- Preserve pipeline snapshots for retry and rollback
- Chain shell commands so variables persist across execution
- Sync versions across config files and fix version-bump stage
- GitHub-release template errors
- TS18047 null-check errors in Settings agent status block

### Changed

- Replace app icon with new Chibby bolt logo across all platforms (macOS, Windows, Linux, web favicon)
- Replace all GitHub URLs from okapian to Nyantahi/chibby
- Replace email contacts with GitHub-based alternatives (Security Advisories, Issues)
- Gitignore `.chibby/` and `.claude/` directories (unique per install)
- Move community files to docs/community/
- Add comparison table and screenshots section to README

### Dependencies

- Downgrade @eslint/js ^10.0.1 → ^9.39.4
- Downgrade typescript ^6.0.2 → ^5.9.3
- Bump @vitest/coverage-v8 ^4.1.0 → ^4.1.1
- Bump happy-dom, react-router-dom, vitest, @vitejs/plugin-react, vite, lucide-react
- Bump Rust crates: reqwest, toml, console, indicatif
- Bump GitHub Actions: checkout v6, setup-node v6, upload-artifact v7, codecov v5, github-script v8

## [0.1.0] - Initial Release

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
- 20 built-in templates: 9 full pipelines + 11 stage snippets
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
- Homebrew Tap Publish template for pushing to custom tap repos
- Homebrew Core PR template for submitting PRs to homebrew-core/homebrew-cask via `brew bump-formula-pr`
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
