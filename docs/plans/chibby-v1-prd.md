# Chibby v1 Product Requirements Document

Status: Draft
Date: 2026-03-16

## Product Summary

Chibby is a local-first, open-source CI/CD and deployment tool for solo
developers and tiny teams. It helps users turn existing scripts into visual,
repeatable pipelines that can run locally or over SSH, with clear logs, run
history, retry, and rollback.

The first release should focus on the simplest valuable workflow:

- import an existing repo
- detect existing scripts
- generate a draft pipeline
- run build/test/deploy locally or over SSH
- manage secrets per environment
- inspect logs and run history
- retry or rollback when a deployment fails

## Problem Statement

Many developers already have working deployment automation in shell scripts,
`Makefile`s, `justfile`s, or package scripts. Existing CI/CD tools often require
too much setup, too much tool-specific learning, or too much infrastructure for
that user profile.

The target user does not want to become a CI administrator. They want a safer,
clearer, more repeatable way to run the automation they already trust.

## Target Users

Primary user:

- solo developer shipping a web app, API, or desktop app
- already uses shell scripts, task runners, or manual deploy commands
- deploys from a laptop to one or more environments
- wants a local-first tool instead of a hosted CI platform

Secondary user:

- tiny engineering team sharing a simple deploy workflow
- self-hosted developers using SSH and lightweight infrastructure
- native app developers with non-container-friendly release flows

## Jobs To Be Done

When I already have scripts that work, I want to import them into a clean tool
that gives me visibility, repeatability, and safer deployment actions without
forcing me to adopt an enterprise CI platform.

When a deployment fails, I want to see exactly what happened, retry only what is
needed, and rollback confidently.

When I work with coding agents or fast iteration loops, I want a simple way to
validate and ship changes without wiring together a server, runners, and hosted
infrastructure.

## Product Goals

### Goal 1: Zero-server first-run value

Users should get value on a single machine with no shared server required.

### Goal 2: Best-in-class script onboarding

Users should be able to start from existing scripts rather than writing a new
pipeline from scratch.

### Goal 3: Native-first execution

Local execution and SSH execution should be first-class. Container execution is
optional.

### Goal 4: Safer deployment operations

Users should be able to inspect logs, see run history, retry from failure, and
rollback to a known good version.

### Goal 5: Open-source usability

The tool should be useful without a hosted dependency and understandable enough
for community contribution.

## Non-Goals for v1

- shared multi-tenant server mode
- Kubernetes orchestration
- runner fleets
- plugin marketplace
- enterprise RBAC
- full Git forge integration
- deep PR review automation
- autonomous agent-driven deployment without explicit user approval

## v1 Scope

### Target Platforms

Chibby must work on all three major desktop platforms from v1:

- **macOS** (primary development platform, Apple Silicon and Intel)
- **Linux** (Ubuntu/Debian and Fedora/RHEL families as reference targets)
- **Windows** (Windows 10 and later, 64-bit)

The Rust core engine and CLI must compile and pass tests on all three
platforms. The Tauri desktop app must build and run on all three platforms.
Platform-specific behavior (keychain, shell, paths) must be abstracted behind
a common interface.

### In Scope

- Tauri desktop application
- shared Rust core execution engine
- local process execution backend
- SSH execution backend
- repo onboarding flow
- detection of common scripts and task files
- draft pipeline generation from detected commands
- minimal editable pipeline definition
- environment management
- secret storage using OS keychain where possible
- live log streaming
- run history
- retry from failed stage
- rollback to previous successful deployment

### Out of Scope

- remote agent orchestration
- shared web dashboard
- advanced artifact registry integrations
- marketplace of reusable actions
- cloud-hosted Chibby service

## Key User Flows

### Flow 1: Import an existing repo

1. User opens Chibby.
2. User selects a local repo.
3. Chibby detects scripts from files such as `deploy.sh`, `Makefile`,
   `justfile`, and package scripts.
4. Chibby suggests a pipeline structure with build, test, and deploy stages.
5. User reviews and saves the pipeline.

Success criteria:

- user can reach a runnable pipeline in less than 10 minutes
- no manual config authoring required for the first run

### Flow 2: Configure environments and secrets

1. User defines environments such as `dev`, `staging`, and `production`.
2. User sets deploy command and target host per environment.
3. User stores required secrets.
4. Chibby validates missing variables before execution.

Success criteria:

- user can manage secrets without storing plaintext in repo config
- missing configuration is surfaced before a run starts

### Flow 3: Run locally or deploy over SSH

1. User selects pipeline and target environment.
2. Chibby runs stages locally or over SSH based on step configuration.
3. The UI streams logs and step status in real time.
4. Run metadata is saved on completion.

Success criteria:

- user can see per-step status and raw command output
- local and SSH execution behave consistently from the user's perspective

### Flow 4: Recover from failure

1. A stage fails.
2. Chibby shows the failing command, logs, and failure summary.
3. User chooses retry from failed stage or rollback.
4. Chibby executes the selected action and updates run history.

Success criteria:

- users do not need to rerun the full pipeline for every failure
- rollback is explicit and auditable

## Functional Requirements

### Repo Onboarding

- User can select a local repository from disk.
- System detects common script sources and task files.
- System can infer an initial pipeline graph from detected commands.
- User can edit stage names, order, and commands before saving.

### Pipeline Model

- Pipeline supports ordered stages.
- Each stage supports one or more commands.
- Each stage can target a backend: `local` or `ssh`.
- Pipeline supports environment-specific variables and secrets.
- Pipeline format is TOML, chosen for alignment with the Rust ecosystem, human
  readability, and avoidance of YAML indentation pitfalls.
- Pipeline files are stored as `.chibby/pipeline.toml` in the repo root.

### Execution Engine

- Run commands as child processes for local backend.
- Run commands over SSH for remote backend.
- Stream stdout and stderr to the UI.
- Capture exit codes, timestamps, durations, and final status.
- Persist run records locally.
- On macOS and Linux, execute commands through the user's default shell
  (`/bin/sh` or as configured).
- On Windows, execute commands through `cmd.exe` by default with optional
  PowerShell support. Detect and support WSL when available.
- Normalize line endings and exit code semantics across platforms.
- Handle platform-specific path separators transparently (`/` vs `\`).

### Logs and Run History

- Show live logs per stage.
- Show completed runs with timestamp, repo, branch if available, environment,
  commit if available, and final status.
- Allow users to inspect historical logs.
- Clearly mark the last successful deployment per environment.

### Retry and Rollback

- Retry can start from the failed stage.
- Rollback can invoke an explicit rollback command or rerun a designated
  previous release action.
- Rollback must require a user action and be visible in history.

### Secrets and Environment Management

- Support per-environment secrets.
- Use platform-native credential storage:
  - macOS: Keychain Services (via `security` CLI or `keychain-services` crate)
  - Linux: `libsecret` / GNOME Keyring / KDE Wallet (via `keyring` crate)
  - Windows: Windows Credential Manager (via `keyring` crate)
- Show missing required secrets before execution.
- Never store secret values in exported logs.
- Fall back to encrypted local file storage if no platform keychain is
  available (e.g. headless Linux server without a desktop session).

### Git Awareness

- Detect current branch and commit when available.
- Associate run records with branch and commit metadata.
- Support manual run as the primary trigger.
- Support optional local Git-triggered runs later in v1 if low complexity.

### Native App Release Support

- Collect build artifacts per run (`.dmg`, `.msi`, `.AppImage`, `.app`,
  `.exe`, `.deb`, `.rpm`, `.nsis`, etc.).
- Store artifact paths and checksums in run history.
- Support platform-specific code signing credential references:
  - macOS: Apple Developer ID and team identifiers
  - Windows: Authenticode certificate (`.pfx` / hardware token)
  - Linux: GPG signing keys for package repositories
- Support macOS notarization as an async post-build step with status polling.
- Support Windows SmartScreen reputation awareness (signed vs unsigned
  guidance).
- Track version identifiers per build and flag version mismatches across
  config files (e.g. `package.json` vs `tauri.conf.json`).
- Archive artifacts per environment and deployment so rollback can reference
  a specific build output.
- Support platform-specific packaging formats:
  - macOS: `.dmg`, `.pkg`, `.app` bundle
  - Windows: `.msi`, `.exe` (NSIS), `.msix`
  - Linux: `.deb`, `.rpm`, `.AppImage`, `.flatpak`, Snap

### Docker-over-SSH Deployment

- Support `docker compose` commands as first-class deploy steps over SSH.
- Support pre-deploy and post-deploy health check commands.
- Support service-level granularity for multi-service Docker deployments.
- Support rollback by redeploying a previous image tag or compose config.

### Data and Configuration Model

- Pipeline configuration is stored in `.chibby/pipeline.toml` inside the
  repo and is intended to be checked into version control.
- Secret references are stored in `.chibby/secrets.toml` as name-only
  entries (no values). This file can be checked into version control.
- Secret values are stored in the OS keychain, keyed by project and
  environment.
- Run history, logs, and artifact paths are stored outside the repo in
  platform-appropriate application data directories:
  - macOS: `~/Library/Application Support/Chibby/`
  - Linux: `~/.local/share/chibby/` (XDG)
  - Windows: `%APPDATA%\Chibby\`
- Environment definitions are stored in `.chibby/environments.toml` inside
  the repo.
- The `.chibby/` directory structure:
  ```
  .chibby/
    pipeline.toml       # stages, commands, backends
    environments.toml   # environment targets and variables
    secrets.toml        # secret name references (no values)
  ```

### Optional v1.1 Agent Assistance

- Summarize failure output in plain English.
- Suggest missing dependencies or secrets.
- Detect repeated failures and flaky steps.

Note: LLM-assisted pipeline generation from repo contents is part of the v1
onboarding flow (see Phase 2 in the roadmap), not a v1.1 feature.

These capabilities must remain optional and must not obscure raw logs.

## UX Requirements

- First-run flow must be understandable without reading documentation.
- Pipeline graph must be visually simple and readable.
- Deployment targets must be obvious and hard to confuse.
- Failure state must clearly show which command failed and what can be done next.
- Retry and rollback actions must be easy to find but not easy to trigger by
  accident.
- UI should feel closer to a deployment console than an enterprise dashboard.

## Technical Requirements

- Core automation logic must live outside the UI in a reusable Rust engine.
- Desktop app must be built with Tauri.
- Core engine should expose interfaces that can later support CLI use.
- Pipeline state and run history must persist locally.
- System must work offline for local and SSH workflows.
- Design must allow optional remote agents later without rewriting core
  execution semantics.
- The Rust core engine must compile on macOS, Linux, and Windows.
- CI must run tests on all three platforms before release.
- Platform-specific code must be isolated behind trait abstractions:
  - shell execution (sh/bash vs cmd/PowerShell)
  - credential storage (Keychain vs libsecret vs Credential Manager)
  - file paths and data directories
  - process management and signal handling
- The Tauri app must use platform-native window chrome and system tray
  integration where available.
- SSH functionality must work identically from all three host platforms.

## Security and Privacy Requirements

- Secrets should use platform-native credential storage where available
  (Keychain on macOS, libsecret on Linux, Credential Manager on Windows).
- Secret values must be redacted from logs where feasible.
- SSH credentials should not be duplicated unnecessarily.
- The system should default to local-only trust assumptions in v1.
- Any future agent or AI features must be explicit, reviewable, and optional.
- On Linux, respect XDG directory conventions and file permissions.
- On Windows, respect `%APPDATA%` conventions and avoid storing sensitive
  data in world-readable locations.

## Success Metrics

### Product Metrics

- time to first successful run
- time to first successful deploy
- percentage of users who import existing scripts without manual pipeline authoring
- retry success rate after pipeline failure
- rollback success rate

### User Value Metrics

- reduced manual deploy steps
- reduced need to open terminal history for common deploy tasks
- perceived clarity of deployment history and failure causes

## Release Criteria for v1

- user can import a repo and run a detected pipeline locally
- user can configure at least one SSH deployment target
- user can store and use secrets without plaintext repo storage
- user can inspect live logs and historical run records
- user can retry from a failed stage
- user can perform a rollback action
- core workflow works for at least one Tauri-style native release scenario and
  one typical web app deploy scenario
- the Tauri desktop app builds and runs on macOS, Linux, and Windows
- the Rust core engine and CLI pass tests on all three platforms
- platform-native credential storage works on all three platforms
- SSH execution works from all three host platforms

## Risks

- script detection may be too ambiguous across different repos
- rollback semantics may vary too much unless clearly defined
- cross-platform secret and SSH UX may be uneven
- native desktop release workflows may expose OS-specific edge cases early,
  particularly macOS notarization and Windows code signing
- cross-platform builds require access to each target OS, which may push
  remote agent support earlier than planned
- scope can drift into building a full CI platform instead of a focused deploy
  tool

## Open Questions

- Should rollback be command-based only in v1, or also artifact-aware?
- How much Git-trigger automation belongs in v1 versus post-v1?
- Does the first release need container execution at all?
- Can Chibby orchestrate builds across multiple machines (e.g. SSH to a Mac
  mini for macOS builds, or to a Windows machine for `.msi` signing)? This
  may fall under Phase 4 (SSH backends) or Phase 8 (remote agents). For v1,
  the baseline is building on the machine the user is sitting at.
- What is the minimum viable notarization UX for macOS? Polling with a
  progress indicator, or background completion with notification?

## Resolved Questions

- Pipeline format: **TOML**. Chosen for Rust ecosystem alignment, human
  readability, and avoidance of YAML indentation pitfalls. Stored as
  `.chibby/pipeline.toml`.
- LLM-assisted pipeline generation timing: included in **v1 onboarding**
  (Phase 2), not deferred to a later agent phase. Falls back to heuristics
  when LLM access is unavailable.

## Dependencies

- Tauri desktop shell (macOS, Linux, Windows)
- Rust process execution and SSH integration
- local storage layer for runs and configuration
- Platform-native credential storage libraries:
  - macOS: Keychain Services
  - Linux: `libsecret` / Secret Service API
  - Windows: Windows Credential Manager
- lightweight parser or detector for common script/task sources
- Cross-platform CI for running tests on macOS, Linux, and Windows (GitHub
  Actions matrix or equivalent)
- Platform-specific Tauri build dependencies:
  - macOS: Xcode Command Line Tools
  - Linux: `libwebkit2gtk`, `libssl-dev`, `libgtk-3-dev`, and related
    development packages
  - Windows: Microsoft Visual Studio C++ Build Tools, WebView2

## Document Relationships

This PRD is derived from:

- [Chibby concept doc](./chibby-local-first-ci-cd.md)
- current OSS CI/CD market research captured in that document
