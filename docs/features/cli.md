# Chibby CLI

A standalone command-line interface that shares the same engine as the desktop app. Designed for headless servers, scripting, and terminal-first workflows.

## Installation

Build the CLI with:

```bash
cd chibby/src-tauri
cargo build --features cli --bin chibby-cli --release
```

The binary is at `target/release/chibby-cli`. Copy it to your PATH:

```bash
# Install as 'chibby' command
cp target/release/chibby-cli /usr/local/bin/chibby
```

## Usage

Running `chibby` without arguments displays an ASCII banner:

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

## Commands

| Command | Description |
|---------|-------------|
| `run` | Run the pipeline for the current project |
| `status` | Show status of the current or last run |
| `cancel` | Cancel a running pipeline |
| `projects` | Manage projects (list, add, remove, info) |
| `pipeline` | Manage pipelines (generate, validate, show, edit) |
| `history` | View run history |
| `retry` | Retry a failed run |
| `rollback` | Rollback to a previous successful run |
| `secrets` | Manage environment variables and secrets |
| `env` | Manage environments (list, show, test) |
| `version` | Version management (show, bump) |
| `artifact` | Artifact management (list, collect, clean) |
| `scan` | Security and quality scans (secrets, deps, commits) |
| `preflight` | Run preflight checks |
| `updater` | Tauri updater commands |
| `init` | Initialize a new project |
| `logs` | Stream logs from a run |
| `app` | Open the desktop app |

## Global Options

| Flag | Description |
|------|-------------|
| `-v, --verbose` | Enable verbose output |
| `--no-color` | Disable colors and emoji |
| `--json` | Output as JSON (for scripting) |

## Examples

### Run Pipeline

```bash
# Run in current directory
chibby run

# Run with environment
chibby run --env production

# Dry run (preview)
chibby run --dry-run

# Run specific stages
chibby run --stage build --stage test
```

### Project Management

```bash
# List all projects
chibby projects list

# Add a project
chibby projects add ~/my-project --name "My App"

# Show project info
chibby projects info
```

### Pipeline Management

```bash
# Generate pipeline from detected scripts
chibby pipeline generate

# Generate with AI assistance
chibby pipeline generate --ai

# Validate configuration
chibby pipeline validate

# Show stages
chibby pipeline show
```

### Secrets

```bash
# Check secrets status
chibby secrets status

# Set a secret
chibby secrets set DEPLOY_KEY

# Delete a secret
chibby secrets delete DEPLOY_KEY
```

### Security Scans

```bash
# Scan for leaked secrets
chibby scan secrets

# Scan dependencies
chibby scan deps

# Lint commits
chibby scan commits
```

### Initialize New Project

```bash
# Basic initialization
chibby init

# Initialize with AI-generated pipeline
chibby init --ai
```

## Output Styling

The CLI uses consistent colors for better readability:

- **Green**: Success, passed, configured
- **Red**: Failed, errors
- **Blue/Cyan**: Running, in progress
- **Yellow**: Warnings, skipped, cancelled
- **Magenta**: Secrets, sensitive data

Spinners and progress bars provide feedback during operations.

## Architecture

The CLI shares the same Rust engine (`chibby_lib`) as the desktop app. Data is stored in the same location (`~/Library/Application Support/chibby` on macOS), so projects and history are shared between CLI and GUI.

### Feature Flags

- `gui` (default): Includes Tauri plugins for desktop app
- `cli`: Includes CLI dependencies (clap, owo-colors, console, indicatif)

Build CLI-only:
```bash
cargo build --no-default-features --features cli --bin chibby-cli
```

## Scripting

Use `--json` for machine-readable output:

```bash
chibby status --json | jq '.status'
```

Use `--no-color` to disable ANSI codes in pipes:

```bash
chibby run --no-color > build.log
```
