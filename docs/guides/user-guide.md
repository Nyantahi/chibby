# Chibby User Guide

This guide walks you through using Chibby to manage CI/CD pipelines for your projects.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Adding a Project](#adding-a-project)
3. [Understanding the Dashboard](#understanding-the-dashboard)
4. [Working with Pipelines](#working-with-pipelines)
5. [Running Pipelines](#running-pipelines)
6. [Viewing Run History](#viewing-run-history)
7. [Pipeline Configuration](#pipeline-configuration)
8. [Command Line Interface (CLI)](#command-line-interface-cli)
9. [Troubleshooting](#troubleshooting)

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

### Step 1: Open Add Project

From the Dashboard, click the **Add Project** button in the top right corner.

### Step 2: Select Repository

Click **Browse** to open the file picker and navigate to your project's root directory. Select the folder containing your source code.

### Step 3: Script Detection

Chibby automatically scans for:

- Shell scripts: `deploy.sh`, `build.sh`, `release.sh`
- Task runners: `Makefile`, `justfile`
- Container files: `Dockerfile`, `docker-compose.yml`
- Package scripts: `package.json` scripts, `Cargo.toml`

Detected scripts appear in a list showing the file name, path, and type.

### Step 4: Generate Pipeline

Click **Generate Pipeline** to create a draft pipeline from detected scripts. Chibby analyzes your project structure and suggests appropriate stages.

### Step 5: Review and Save

Review the generated pipeline. You can:

- Edit stage names
- Modify commands
- Reorder stages
- Remove unnecessary stages
- Add new stages

Click **Save Pipeline** to finalize. The pipeline is stored as `.chibby/pipeline.toml` in your project.

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

To modify a pipeline, edit the `.chibby/pipeline.toml` file directly in your code editor. The format is human-readable TOML:

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

## Command Line Interface (CLI)

Chibby includes a standalone CLI that shares data with the desktop app. Use it for headless servers, scripting, and terminal-first workflows.

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
