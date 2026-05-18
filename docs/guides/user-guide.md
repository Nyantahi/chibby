# Chibby User Guide

This guide walks you through using Chibby to manage CI/CD pipelines for your projects.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Adding a Project](#adding-a-project)
3. [Understanding the Dashboard](#understanding-the-dashboard)
4. [Project Detail tabs](#project-detail-tabs)
5. [Working with Pipelines](#working-with-pipelines)
6. [Pipeline Templates](#pipeline-templates)
7. [Running Pipelines](#running-pipelines)
8. [Viewing Run History](#viewing-run-history)
9. [Environments tab](#environments-tab)
10. [Release tab](#release-tab)
11. [Quality tab](#quality-tab)
12. [Pipeline Configuration](#pipeline-configuration)
13. [App Settings](#app-settings)
14. [Crash log](#crash-log)
15. [Command Line Interface (CLI)](#command-line-interface-cli)
16. [Troubleshooting](#troubleshooting)

---

## Getting Started

### Launching Chibby

After installation, launch Chibby from your applications menu:

- **macOS**: Open from Applications or Spotlight
- **Linux**: Find Chibby in your application launcher, or run the AppImage directly
- **Windows**: Launch from the Start menu

For development mode, run `npm run tauri:dev` from the project directory.

The main window opens to the Dashboard showing all your tracked projects.

### First Time Setup

No additional configuration is required. Chibby stores all data locally:

- Pipeline configurations live in each project's `.chibby/` directory
- Run history is stored in your system's application data folder
- Secrets are stored in your OS keychain (not on disk)

See the [installation guide](installation.md) for platform-specific setup
details including SSH configuration and keychain access.

---

## Adding a Project

The Add Project wizard walks you through a 5-step flow to set up a new project with a pipeline and optional deployment.

### Step 1: Select Repository

From the Dashboard, click **Add Project**. Click **Browse** to pick your project's root directory, or type the path directly. Optionally set a custom project name.

Click **Scan Repository** to detect build files and continue.

### Step 2: Choose Pipeline Source

After scanning, Chibby shows what it found (build files, GitHub Actions workflows) and offers three options:

- **Auto-detect** — Generate a pipeline from detected scripts (`Makefile`, `package.json`, `Cargo.toml`, `deploy.sh`, `Dockerfile`, etc.)
- **GitHub Actions** — Import stages from your existing `.github/workflows/` files. Only shown if workflows are detected.
- **From Template** — Browse the built-in and custom template library to start from a proven pipeline configuration.

If you arrived here from the **Templates** page via "Apply Template" or "Use as Starting Point", the template is pre-selected and shown as a banner. Just scan the repo and the template variable dialog opens automatically.

### Step 3: Configure Stages

Toggle individual stages on or off. When using GitHub Actions import, Chibby may suggest additional stages based on detected build files that aren't covered by the imported workflow.

### Step 4: Deploy

Pick a deployment method (Docker Compose over SSH, GitHub Release, Vercel, Fly.io, Railway, etc., or **Skip** for CI-only). Fill in any required fields — SSH host, registry, health-check URL, platform project name. The selected method generates a separate `deploy.toml` pipeline alongside `pipeline.toml`.

### Step 5: Review and Create

Review the final project setup showing project name, path, source method, stage count, deploy target, and all selected stages. Click **Create Project** to save. The pipeline is stored as `.chibby/pipeline.toml` in your project.

After creation, Chibby runs **auto-bootstrap** — it scans your repo for env variable and secret references and either pops the [Bootstrap wizard](#bootstrap-wizard) (default `confirm` mode), silently writes `environments.toml` + `secrets.toml` (`silent` mode), or does nothing (`off` mode). Change this from [App Settings](#app-settings).

---

## Understanding the Dashboard

The Dashboard displays all tracked projects as cards.

### Project Card Elements

Each card shows:

- **Project name** - The repository folder name
- **Path** - Full path to the project
- **Pipeline status** - "Pipeline configured" badge if a pipeline exists
- **Last run** - Status icon and timestamp of the most recent run

### Status Icons

| Icon | Meaning |
|------|---------|
| Green checkmark | Last run succeeded |
| Red X | Last run failed |
| Blue circle | Run in progress |
| Gray circle | No runs yet / pending |

### Navigation

Click any project card to open the Project Detail view.

---

## Project Detail tabs

The Project Detail view is organised into five tabs:

| Tab | What it covers |
|-----|----------------|
| **Pipeline** | The current pipeline's stages, the editor, live run output |
| **History** | Past runs with status, branch, commit, retry, and rollback |
| **Environments** | `environments.toml`, `secrets.toml`, the Bootstrap wizard, importers, leak warnings, and `.env` export |
| **Release** | Version bumping, artifacts, code signing, the Tauri updater, and notifications |
| **Quality** | Security/quality gates, retention cleanup, and deployment history |

The right-hand sidebar is shared across tabs. It always shows project stats, recommendations, the file detector, and a **Quick Links** card with reveal-in-Finder buttons for `.chibby/`, the data directory, and the repo folder.

---

## Working with Pipelines

### Pipeline Structure

A pipeline consists of ordered stages. Each stage contains:

- **Name** - Descriptive label (e.g., "build", "test", "deploy")
- **Commands** - One or more shell commands to execute
- **Backend** - Execution target: `local` or `ssh`
- **Fail fast** - Whether to stop the pipeline if this stage fails

### Viewing Pipeline Stages

In the Project Detail view, the Pipeline section displays all stages as cards. Each card shows:

- Stage number and name
- Backend type badge
- Commands listed as code blocks
- Play button to run that stage individually

### Editing Pipelines

The Pipeline Editor in the Project Detail view lets you modify pipelines visually:

- Add, remove, and reorder stages
- Edit stage names, commands, backend type, and working directory
- Configure health checks per stage
- Drag stages to reorder them

#### Import from GitHub Actions

Click **Import CI** to import stages from your GitHub Actions workflows. A modal shows all workflows from `.github/workflows/` with their jobs and steps. Select the steps you want and they become new pipeline stages.

#### Add Stage Templates

Click **Stage Templates** to browse the template library filtered to stage snippets. Select a template (e.g., Docker Build & Push, S3 Deploy, Version Bump & Tag) and fill in any variables. The stages are appended to your pipeline.

#### Save as Template

Click **Save as Template** to save the current pipeline as a reusable template. Add a name, description, category, and tags, then choose to save it globally (user scope) or per-project. Saved templates appear in the template browser for future use.

You can also edit `.chibby/pipeline.toml` directly in your code editor. The format is human-readable TOML:

```toml
name = "My App Build"

[[stages]]
name = "install"
commands = ["npm install"]
backend = "local"
fail_fast = true

[[stages]]
name = "build"
commands = ["npm run build"]
backend = "local"
fail_fast = true
```

---

## Pipeline Templates

Chibby includes a template system for creating, sharing, and reusing pipeline configurations. For full details, see the [Templates documentation](../features/templates.md).

### Browsing Templates

Navigate to the **Templates** page from the sidebar. The template browser lets you:

- Search by name, description, or tags
- Filter by category (Rust, Node.js, Python, Go, Docker, Deployment)
- Filter by type (Full Pipelines vs Stage Snippets)
- Filter by source (Built-in, User, Project)

Expand any template to preview its stages and required tools.

### Applying a Template

Click **Apply Template** to use a template for a new project. This navigates to the Add Project wizard with the template pre-selected. After selecting a repository, fill in any template variables (e.g., project name, SSH host) and the pipeline is generated.

Click **Use as Starting Point** to do the same but with the intent to customize the pipeline further in the configure step.

### Built-in Templates

Chibby ships with 20 built-in templates:

**Full Pipelines:** Rust CLI, Rust Library, Node.js Web App, Python Django, Python FastAPI, Go Web Service, Static Site, Tauri Desktop, Docker Compose Deploy

**Stage Snippets:** GitHub Release, Docker Build & Push, Docker Compose SSH, SSH Rsync Deploy, Cargo Publish, npm Publish, S3 Deploy, Tauri Bundle, Version Bump & Tag, Homebrew Tap Publish, Homebrew Core PR

### Custom Templates

Save your own templates from the Pipeline Editor ("Save as Template" button) or import templates from TOML files. Templates are stored in:

- **User scope:** `~/.chibby/templates/` (available across all projects)
- **Project scope:** `<repo>/.chibby/templates/` (shareable via version control)

### Template Variables

Templates can include `{{variable}}` placeholders that are filled in when applied. For example, the Version Bump & Tag template includes a `{{bump_level}}` variable that lets you choose between `patch`, `minor`, or `major` version increments.

---

## Running Pipelines

### Run Full Pipeline

1. Navigate to the Project Detail view
2. Click the **Run Pipeline** button in the header
3. Watch stages execute sequentially
4. View live logs in the Run Detail view

### Run Single Stage

To run just one stage:

1. Find the stage card in the Pipeline section
2. Click the small play button on that stage
3. Only that stage executes

This is useful for:

- Testing a specific stage
- Retrying a failed stage without re-running earlier stages
- Running deploy without rebuild

### During Execution

While a pipeline runs:

- The Run button shows "Running..."
- You cannot start another run on the same project
- Logs stream in real time
- Each stage shows its current status

---

## Viewing Run History

### Run History List

Below the Pipeline section, the Run History shows past executions:

| Column | Description |
|--------|-------------|
| Status | Success, failed, or cancelled |
| Started | Timestamp when run began |
| Duration | Total execution time |
| Branch | Git branch if detected |
| Commit | Short commit hash if detected |

### Run Detail View

Click any run to see detailed information:

- Per-stage status and duration
- Full stdout and stderr logs
- Exit codes for each command
- Timestamps for stage start/finish

### Log Viewer

In the Run Detail view:

- Use the stage tabs to switch between stage logs
- Stdout appears in normal text
- Stderr may appear in a different color
- Long logs are scrollable

### Reveal run folder

The Run Detail header has a **Reveal** button that opens `<data_dir>/runs/<run_id>/` in your OS file manager — handy for inspecting raw stage output, attached artifacts, or sharing the folder with a teammate.

---

## Environments tab

Open the **Environments** tab on Project Detail to manage everything in `.chibby/environments.toml`, `.chibby/environments.local.toml`, and `.chibby/secrets.toml`. See the [Environments & Secrets feature doc](../features/env-secrets.md) for the underlying file formats and resolution order.

### Bootstrap & Import bar

A toolbar at the top of the tab exposes three actions:

- **Bootstrap** — opens the Bootstrap wizard for an existing project at any time.
- **Import…** — opens the importer modal to pull names (and optionally values) from a `.env` file, Vercel, Railway, or Fly.io.
- **Export .env** — opens a save dialog and writes the resolved variables + secret values for the selected environment to a flat `.env` file (uses the OS keychain).

### Bootstrap wizard

The wizard runs `scan_bootstrap` on your repo and lists every detected name with:

- **Classification** — secret (keychain) or variable (`environments.toml`), inferred from name segments.
- **Sources** — the files where the name appeared (e.g. `.env.production`, `docker-compose.prod.yml`, `.github/workflows/deploy.yml`).
- **Suggested environments** — extracted from filenames like `.env.staging` → `staging`.

Two apply modes:

| Mode | Behaviour |
|------|-----------|
| **Safe** (Merge checkbox off) | Refuses to write if `environments.toml` or `secrets.toml` already exists |
| **Merge** (default) | Appends only newly-detected names; never modifies existing entries |

The same wizard runs automatically after Add Project unless you set bootstrap mode to `silent` or `off` in App Settings.

### Importer modal

Pick a source, target environment, and (for `dotenv`) the `.env` file to read. The modal probes the vendor CLI first — Vercel/Railway/Fly importers report whether `vercel`, `railway`, or `flyctl` is on PATH before you click Run.

| Checkbox | What it does |
|----------|--------------|
| **Pull values** | Asks the source for plaintext values, not just names. Off = names-only. |
| **Save secret values to keychain** | Persists detected secret values into the OS keychain. Off = secret names land in `secrets.toml` but you set values later via the Secrets card. |

After running, the modal shows variables added, variables valued, secret refs added, and secret values saved.

### Environments card

The same `EnvironmentEditor` you've always had, plus two additions:

- **Mode selector** — switches between editing the **Committed** file, your per-developer **Local overrides** (`environments.local.toml`, auto-gitignored), and a read-only **Layered** view that previews the merged result a run would see.
- **Leak banner** — a red banner appears whenever `scan_environments_for_leaks` finds token-shaped values inside `environments.toml`. Each hit shows env · variable · rule and a redacted preview. Re-runs on every save.

### Secrets card

The existing per-environment Set/Delete workflow, plus a clock icon next to each secret/env pair. Click it to open the **Secret audit modal** showing last-set, last-deleted, set/delete counts, and last provenance (`cli`, `gui`, `import:vercel`, etc.) — handy for "when did I last rotate this?" questions.

---

## Release tab

The **Release** tab stacks four cards, each backed by a corresponding `.chibby/*.toml` file. Open it once your project is past the build/test phase and you want to ship.

### Version card

- Shows every version file detected (`package.json`, `Cargo.toml`, `pyproject.toml`, `VERSION`, …), the resolved current version, the latest git tag, and a consistency badge.
- **Bump patch / minor / major** buttons run `bump_version`. The **Create git tag** checkbox decides whether a tag is created.
- **Generate changelog** lists commits since the latest tag with a one-click **Copy** to clipboard for pasting into release notes.

### Artifacts & Signing card

- **Output directory**, **Retention count**, optional **Upload to** URI, and a **Glob patterns** textarea (one per line).
- **Collect** runs `collect_artifacts` against the configured patterns and writes a manifest with SHA256 hashes.
- **Manifests** table lists all collected manifests with a **Reveal output dir** button per row.
- **Signing** sub-section (inside the same card): toggle Enabled, fill in macOS identity / team ID / Windows cert path / Linux GPG key. Detected signing tools are shown in the card header. Each artifact in the latest manifest gets a per-file **Sign** action.

### Updater card

- Config form: Enabled, **Public key**, **Base URL**, **Publish target** (GitHub Release / S3 / SCP / local), and target-specific fields (`github_repo`, S3 bucket/region/endpoint, SCP destination, or local directory).
- **Keys** sub-panel: **Generate**, **Rotate**, **Delete** the private key in the OS keychain, **Copy pub** to clipboard, or paste an existing private key and **Import** it.
- **Preflight** runs `updater_preflight` and lists any blocking issues.
- **Publish** flow: enter a version, toggle **Dry run**, hit **latest.json** to generate the manifest, and **Publish** (or **Dry run**) to ship.

### Notify card

- Toggle Enabled, **Add target**, pick **desktop** or **webhook**, choose when to fire (`always`, `success`, `failure`).
- **Send test** dispatches a notification via the current config so you can verify before a real run.
- Webhook URLs go in a per-target input (Slack / Discord / generic HTTP).

---

## Quality tab

The **Quality** tab is the operational hygiene layer.

### Gates card

Each gate has three modes: `block` (fail the run), `warn` (log only), or `off`. The card exposes selectors for all seven gates:

- **Secret scanning** — backed by `gitleaks` when available, built-in regex fallback otherwise.
- **Dependency scanning** — auto-picks `cargo audit` / `npm audit` / `pnpm audit` / `pip-audit` based on what's in the repo.
- **Commit lint** — conventional commits.
- **SAST (semgrep)** — static analysis for SQLi, XSS, command injection, dangerous subprocess use, etc.
- **Container scan** — `trivy image` against image refs listed in `container_images` (textarea below the selectors) or detected Dockerfiles.
- **IaC scan** — `trivy config` over Dockerfile / docker-compose / Kubernetes / Terraform / CloudFormation.
- **License check** — `cargo-license` + `license-checker`; flags GPL/AGPL by default (configurable via `license_denylist`).

Plus severity thresholds for dependency audit, SAST, and container scans (one of `low` / `medium` / `high` / `critical` — block only on this level and above).

Buttons:

- **Run all** — runs every enabled gate and shows a single passed/failed summary.
- **Per-gate run buttons** — Secret scan / Dependency audit / Commit lint / SAST / Container / IaC / License — kick off one at a time with scanner output rendered inline.
- **Create secret-scan baseline** — snapshot current findings into a baseline so the gate only flags new leaks going forward.

Each scanner is detected at runtime. If the underlying tool (gitleaks, trivy, semgrep, etc.) isn't installed, the gate returns a non-failing `"(missing)"` result with the install command — gates never crash on "scanner not installed."

For the deep dive (config keys, finding shapes, install hints, pipeline auto-append), see [Security & Quality Gates](../features/security-gates.md).

### Cleanup card

Reflects `.chibby/cleanup.toml`. Set **Artifact retention**, **Run retention**, and toggle **Prune remote Docker images on SSH hosts**. Run with **Dry run** checked first to preview what would be removed.

### Deployment history card

Read-only table per environment showing run id, status, kind (normal/retry/rollback), branch, duration, and a **View** link straight to the Run Detail page.

---

## Pipeline Configuration

### Configuration Files

Chibby stores configuration in the `.chibby/` directory at your project root:

```
your-project/
  .chibby/
    pipeline.toml      # Pipeline stages and commands
    environments.toml  # Environment definitions
    secrets.toml       # Secret references, no values
```

### Pipeline TOML Format

```toml
# Pipeline name shown in UI
name = "Project Build and Deploy"

# First stage
[[stages]]
name = "install"
commands = ["npm install"]
backend = "local"
working_dir = ""        # Optional: relative path for working directory
fail_fast = true        # Stop pipeline if this stage fails

# Second stage
[[stages]]
name = "test"
commands = [
  "npm run lint",
  "npm test"
]
backend = "local"
fail_fast = true

# Third stage with SSH backend (future)
[[stages]]
name = "deploy"
commands = ["./deploy.sh"]
backend = "ssh"
fail_fast = true
```

### Backend Types

| Backend | Behavior |
|---------|----------|
| `local` | Runs commands as child processes on your machine |
| `ssh` | Runs commands on a remote server over SSH (Phase 4) |

### Multiple Commands

A stage can have multiple commands. They execute sequentially:

```toml
[[stages]]
name = "quality-checks"
commands = [
  "npm run lint",
  "npm run type-check",
  "npm test"
]
backend = "local"
fail_fast = true
```

If any command fails (non-zero exit code) and `fail_fast = true`, the pipeline stops.

---

## App Settings

Navigate to the **Settings** page from the sidebar to configure app-wide defaults.

### Notifications

- **Notify on successful runs** — Show a desktop notification when a pipeline run completes successfully
- **Notify on failed runs** — Show a desktop notification when a pipeline run fails

These defaults apply to all projects unless a project has its own `.chibby/notify.toml` configuration.

### Retention

- **Artifact retention** — Number of artifact versions to keep per project (default: 5)
- **Run history retention** — Number of run records to keep per project (default: 50)

After each pipeline run, Chibby automatically prunes old artifacts and run history based on these limits. Per-project `.chibby/cleanup.toml` overrides the app defaults.

### Bootstrap mode

Controls what happens when you add a new project — Chibby can scan the repo for env/secret references and either prompt you, do it silently, or skip:

| Mode | Behaviour |
|------|-----------|
| `confirm` (default) | Open the Bootstrap wizard for review before writing anything |
| `silent` | Scan and write `environments.toml` + `secrets.toml` immediately, with a confirmation toast |
| `off` | Skip the scan entirely |

You can always run the wizard manually later from the Environments tab's **Bootstrap** button.

### About

- **Version** — Current app version.
- **Data directory** — Where run history, settings, and the keychain audit live. The folder icon opens it in your OS file manager.
- **View crash log** — Jumps to the [Crash log](#crash-log) page if a crash has been recorded.

---

## Crash log

Visit `/crashes` (or follow Settings → About → View crash log) to inspect `<data_dir>/crash.log`. The page shows the file's content inline with two actions:

- **Reveal in Finder** opens the file in your OS file manager.
- **Clear** deletes the file after confirmation.

If no crash has been recorded, the page just shows "No crash log present."

---

## Command Line Interface (CLI)

Chibby includes a standalone CLI that shares data with the desktop app. Use it for headless servers, scripting, and terminal-first workflows.

The full CLI reference lives in [features/cli-commands.md](../features/cli-commands.md). This section covers the most common workflows; consult the reference for every flag and example.

### Installation

Build the CLI:

```bash
cd chibby/src-tauri
cargo build --features cli --bin chibby-cli --release
cp target/release/chibby-cli /usr/local/bin/chibby
```

### Quick Start

Run `chibby` without arguments to see the ASCII banner:

```
     _____ _     _ _     _           
    / ____| |   (_) |   | |          
   | |    | |__  _| |__ | |__  _   _ 
   | |    | '_ \| | '_ \| '_ \| | | |
   | |____| | | | | |_) | |_) | |_| |
    \_____|_| |_|_|_.__/|_.__/ \__, |
                                __/ |
    local-first CI/CD          |___/ 
```

### Global Options

| Flag | Description |
|------|-------------|
| `-v, --verbose` | Enable verbose output |
| `--no-color` | Disable colors and emoji |
| `--json` | Output as JSON (for scripting) |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### Pipeline Commands

#### `chibby run`

Run the pipeline for the current project.

```bash
# Run in current directory
chibby run

# Run with environment
chibby run --env production

# Dry run (preview without executing)
chibby run --dry-run

# Run specific stages
chibby run --stage build --stage test

# Skip preflight checks
chibby run --skip-preflight

# Run for a different project
chibby run --project ~/my-app
```

#### `chibby status`

Show status of the current or last run.

```bash
chibby status
chibby status --project ~/my-app
```

#### `chibby cancel`

Cancel a running pipeline.

```bash
chibby cancel
chibby cancel --project ~/my-app
```

#### `chibby logs`

Stream logs from a run.

```bash
# View logs for a specific run
chibby logs abc123

# Follow logs in real-time
chibby logs abc123 --follow
```

#### `chibby history`

View run history.

```bash
# Show last 10 runs
chibby history

# Filter by environment
chibby history --env production

# Show more runs
chibby history --limit 25
```

#### `chibby retry`

Retry a failed run.

```bash
# Retry from the beginning
chibby retry abc123

# Retry from a specific stage
chibby retry abc123 --from-stage deploy
```

#### `chibby rollback`

Rollback to a previous successful run.

```bash
chibby rollback abc123
```

### Project Commands

#### `chibby projects list`

List all tracked projects.

```bash
chibby projects list
```

Output shows status icons:
- Green checkmark: Last run succeeded
- Red X: Last run failed
- Gray circle: No pipeline configured

#### `chibby projects add`

Add a project to Chibby.

```bash
chibby projects add ~/my-project
chibby projects add ~/my-project --name "My App"
```

#### `chibby projects remove`

Remove a project from tracking.

```bash
chibby projects remove my-project
```

#### `chibby projects info`

Show project details.

```bash
chibby projects info
chibby projects info my-project
```

#### `chibby init`

Initialize a new project with pipeline configuration.

```bash
# Basic initialization
chibby init

# Initialize with AI-generated pipeline
chibby init --ai

# Initialize a specific directory
chibby init ~/new-project
```

### Pipeline Management

#### `chibby pipeline show`

Display pipeline stages.

```bash
chibby pipeline show
```

#### `chibby pipeline generate`

Generate pipeline from detected scripts.

```bash
# Generate from detected scripts
chibby pipeline generate

# Generate with AI assistance
chibby pipeline generate --ai
```

#### `chibby pipeline validate`

Validate pipeline configuration.

```bash
chibby pipeline validate
```

#### `chibby pipeline edit`

Open pipeline in your editor ($EDITOR).

```bash
chibby pipeline edit
```

### Security and Quality Scans

#### `chibby scan secrets`

Scan for leaked secrets in your repository.

```bash
chibby scan secrets

# Create baseline from current findings
chibby scan secrets --baseline
```

#### `chibby scan deps`

Scan dependencies for vulnerabilities.

```bash
chibby scan deps

# Set minimum severity threshold
chibby scan deps --severity critical
```

#### `chibby scan commits`

Lint commit messages for conventional format.

```bash
chibby scan commits

# Check commits since a tag
chibby scan commits --since v1.0.0
```

#### `chibby scan sast`

Static analysis via semgrep.

```bash
chibby scan sast
```

Requires `semgrep` on PATH (`brew install semgrep` or `pip install semgrep`).

#### `chibby scan container`

Scan container images for OS + app vulnerabilities via `trivy image`. Image refs come from `gates.toml` (`container_images`); when empty, Chibby falls back to Dockerfiles auto-detected in the repo.

```bash
chibby scan container
```

Requires `trivy` (`brew install trivy`).

#### `chibby scan iac`

Scan Dockerfile / docker-compose / Kubernetes / Terraform / CloudFormation for misconfigurations via `trivy config`.

```bash
chibby scan iac
```

Requires `trivy`.

#### `chibby scan license`

Flag GPL/AGPL (or any denylisted licenses) in dependency manifests.

```bash
chibby scan license
```

Requires `cargo-license` for Rust and/or `license-checker` for npm (`cargo install cargo-license` and `npm i -g license-checker`).

#### `chibby preflight`

Run all preflight checks.

```bash
chibby preflight
chibby preflight --env production
```

### Secrets Management

#### `chibby secrets status`

Check which secrets are configured.

```bash
chibby secrets status
```

#### `chibby secrets set`

Set a secret value (stored in OS keychain).

```bash
# Prompt for value securely
chibby secrets set DEPLOY_KEY

# Provide value directly
chibby secrets set DEPLOY_KEY "my-secret-value"
```

#### `chibby secrets delete`

Delete a secret.

```bash
chibby secrets delete DEPLOY_KEY
```

#### `chibby secrets rotate`

Re-prompt for a value and overwrite the existing keychain entry. Other developers' keychains aren't affected — they rotate independently.

```bash
chibby secrets rotate STRIPE_KEY --env production
```

### Bootstrap, Import, and Export

#### `chibby bootstrap`

Scan the current project for env/secret references and populate `.chibby/environments.toml` + `.chibby/secrets.toml`. Names only — values stay empty.

```bash
# Preview without writing
chibby bootstrap --dry-run

# Default: refuses to write if configs already exist
chibby bootstrap

# Merge: append only newly-detected names
chibby bootstrap --merge

# Skip the review table (still writes)
chibby bootstrap --silent
```

The same scanner runs automatically after `chibby projects add` (and the Add Project wizard) when `bootstrap_mode = "confirm"` or `"silent"`.

#### `chibby import`

Pull names (and optionally values) from an external source. Vercel/Railway/Fly require the vendor CLI installed and authenticated.

```bash
# Pull a .env file end-to-end (vars to environments.toml, secrets to keychain)
chibby import dotenv .env.production --env production --with-values

# Bring Vercel's production env in
chibby import vercel --env production --with-values

# Railway
chibby import railway --env production --with-values

# Fly.io — names only (Fly's secrets API is write-only)
chibby import fly --env production
```

#### `chibby export dotenv`

Round-trip — emit a flat `.env` file from Chibby's resolved variables and secret values.

```bash
chibby export dotenv --env production --out .env.production.local
```

### Environment Management

#### `chibby env list`

List configured environments.

```bash
chibby env list
```

#### `chibby env show`

Show environment details.

```bash
chibby env show production
```

#### `chibby env test`

Test SSH connection to an environment.

```bash
chibby env test staging
```

#### `chibby env vars`

Set, get, list, and delete non-secret variables for an environment. Pass `--local` to write to `environments.local.toml` instead of the committed file (auto-gitignored on save).

```bash
chibby env vars set production API_URL https://api.example.com
chibby env vars set production DEBUG true --local
chibby env vars list production
chibby env vars get production API_URL
chibby env vars delete production API_URL
```

#### `chibby env diff`

Show variable + secret deltas between two environments. `+` only in destination, `-` only in source, `~` value differs.

```bash
chibby env diff production staging
```

#### `chibby env scan-leaks`

Run the in-process leak scanner against `environments.toml`. Non-zero exit code on any hit — suitable for a pre-commit hook. Output is always redacted.

```bash
chibby env scan-leaks
```

### Audit & Doctor

#### `chibby audit list`

Per-project secret lifecycle summary: set/delete counts, last action, last provenance (cli / gui / import:vercel / …).

```bash
chibby audit list
```

#### `chibby audit show`

Full audit snapshot for a single secret in a single environment.

```bash
chibby audit show STRIPE_KEY --env production
```

#### `chibby doctor`

End-to-end project health check: config files present, SSH hosts reachable, every declared secret has a value in the keychain for every environment it applies to. Non-zero exit on any failure — wire it into CI before `chibby run --env production`.

```bash
chibby doctor
chibby doctor -p ~/my-project
```

### Version Management

#### `chibby version show`

Show current project version.

```bash
chibby version show
```

#### `chibby version bump`

Bump the project version.

```bash
# Bump patch version (1.2.3 -> 1.2.4)
chibby version bump patch

# Bump minor version (1.2.3 -> 1.3.0)
chibby version bump minor

# Bump major version (1.2.3 -> 2.0.0)
chibby version bump major

# Set explicit version
chibby version bump 2.0.0

# Create git tag
chibby version bump patch --tag

# Generate changelog
chibby version bump minor --changelog
```

### Artifact Management

#### `chibby artifact list`

List build artifacts.

```bash
chibby artifact list
```

#### `chibby artifact collect`

Collect artifacts from last build.

```bash
chibby artifact collect
```

#### `chibby artifact clean`

Clean old artifacts.

```bash
# Preview what would be deleted
chibby artifact clean --dry-run

# Actually clean
chibby artifact clean
```

### Tauri Updater Commands

#### `chibby updater generate-keys`

Generate Tauri update signing keys.

```bash
chibby updater generate-keys
```

#### `chibby updater sign`

Sign an update bundle.

```bash
chibby updater sign path/to/bundle.tar.gz
```

#### `chibby updater latest-json`

Generate latest.json for update server.

```bash
chibby updater latest-json
```

#### `chibby updater publish`

Publish update to distribution server.

```bash
# Preview without publishing
chibby updater publish --dry-run

# Actually publish
chibby updater publish
```

### Other Commands

#### `chibby app`

Open the Chibby desktop app.

```bash
chibby app
```

### Output Styling

The CLI uses consistent colors for readability:

| Color | Meaning |
|-------|--------|
| Green | Success, passed, configured |
| Red | Failed, errors |
| Blue/Cyan | Running, in progress |
| Yellow | Warnings, skipped, cancelled |
| Magenta | Secrets, sensitive data |

### Scripting

Use `--json` for machine-readable output:

```bash
chibby status --json | jq '.status'
```

Use `--no-color` to disable ANSI codes:

```bash
chibby run --no-color > build.log 2>&1
```

### Data Sharing

The CLI and desktop app share the same data directory:

- **macOS**: `~/Library/Application Support/chibby`
- **Linux**: `~/.local/share/chibby`
- **Windows**: `%APPDATA%\chibby`

Projects, run history, and settings are accessible from both interfaces.

---

## Troubleshooting

### Common Issues

#### "No scripts detected"

**Cause**: Chibby cannot find recognized script files.

**Solution**: Ensure your project has one of:

- `deploy.sh`, `build.sh`, or other `.sh` files
- `Makefile` or `justfile`
- `package.json` with scripts
- `Dockerfile` or `docker-compose.yml`

#### Pipeline won't start

**Cause**: Another run may be in progress.

**Solution**: Wait for the current run to complete or cancel it.

#### Commands fail with "command not found"

**Cause**: The command is not in your system PATH.

**Solution**:

- Use full paths in your pipeline commands
- Ensure required tools are installed
- Check that your shell profile loads the correct PATH

#### Logs don't appear

**Cause**: The process may be buffering output.

**Solution**:

- Commands will show output when they complete
- Long-running commands may take time to flush output

### Getting Help

- Check the [README](../README.md) for setup instructions
- Review [pipeline examples](../../docs/phase-0-audit.md) for common patterns
- See the [changelog](../../CHANGELOG.md) for feature history

### Reset Chibby Data

To clear all run history and start fresh:

**macOS**:

```bash
rm -rf ~/Library/Application\ Support/Chibby/
```

**Linux**:

```bash
rm -rf ~/.local/share/chibby/
```

**Windows**:

```powershell
Remove-Item -Recurse -Force "$env:APPDATA\Chibby\"
```

Pipeline configurations remain in each project's `.chibby/` folder.

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + R` | Refresh current view |
| `Esc` | Close dialogs |

---

## What's Next

See the [changelog](../../CHANGELOG.md) for completed features and recent changes.
