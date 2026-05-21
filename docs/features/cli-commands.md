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

### Environments

```bash
# List environments (merged with environments.local.toml overrides)
chibby env list

# Show one environment, including resolved variables
chibby env show production

# Add a new environment
chibby env add production --ssh-host deploy@prod.example.com --ssh-port 22

# Duplicate an environment
chibby env copy production staging

# Open environments.toml in $EDITOR (validates on save)
chibby env edit

# Test SSH connectivity for an environment
chibby env test production

# Remove an environment
chibby env remove staging

# Diff two environments (variables + secret references)
chibby env diff production staging

# Scan environments.toml for variable values that look like real credentials
chibby env scan-leaks
```

`env diff` legend: `+` only in destination, `-` only in source, `~` value differs. Pure differences only — identical entries are summarised as "identical."

`env scan-leaks` is opportunistic — it flags values matching common token shapes (GitHub PATs, OpenAI/Anthropic/Stripe/Slack/AWS keys, db URLs with embedded credentials, private-key blocks). Output is redacted to never echo the suspect value verbatim. Non-zero exit when matches are found, suitable for a pre-commit hook.

### Environment Variables

Non-secret config values per environment. Pair with `secrets` for sensitive values.

```bash
# Set a variable (writes to environments.toml — committed)
chibby env vars set production API_URL https://api.example.com

# Set a per-developer override (writes to environments.local.toml — gitignored)
chibby env vars set production API_URL https://localhost:8000 --local

# List variables for an environment (shows the merged view)
chibby env vars list production

# Read a single value (suitable for shell substitution)
chibby env vars get production API_URL

# Delete a variable
chibby env vars delete production API_URL
```

### Secrets

Secrets live in two places: declared references in `.chibby/secrets.toml` (committed, names only) and values in the OS keychain (never written to disk).

```bash
# List declared secret references
chibby secrets list

# Add a new reference (optionally scope to specific environments)
chibby secrets add DEPLOY_KEY --env production --env staging

# Set a value (prompts securely if --value omitted)
chibby secrets set DEPLOY_KEY --env production

# Set non-interactively (e.g. in a setup script)
chibby secrets set DEPLOY_KEY --env production --value "$DEPLOY_KEY_FROM_PARENT_SHELL"

# Rotate (alias for set with prompt — emphasises intent)
chibby secrets rotate DEPLOY_KEY --env production

# Check which secrets are set per environment
chibby secrets status                    # all declared envs
chibby secrets status --env production   # one environment

# Delete a value from the keychain (does not remove the reference)
chibby secrets delete DEPLOY_KEY --env production

# Remove a reference from secrets.toml (keychain values are NOT auto-deleted)
chibby secrets remove DEPLOY_KEY
```

### Audit

Per-secret lifecycle metadata: how many times each secret has been set/deleted, last action, and where (CLI / GUI / which importer). Stored under `<chibby_data_dir>/secret_audit/<repo_hash>.json` — follows the user's Chibby install, not the repo.

```bash
# Project-wide summary, one line per secret
chibby audit list

# Detailed snapshot for one secret
chibby audit show STRIPE_KEY --env production
```

Audit records are written best-effort — failures are logged but never block the underlying secret operation.

### Doctor

End-to-end diagnostic — config files present, SSH reachable, all declared secrets resolved in the keychain.

```bash
chibby doctor                 # current directory
chibby doctor -p /path/to/project
```

Exits non-zero if any check fails — suitable for CI gating before a deploy.

### Import

Pull env/secret references from external sources into Chibby's configs. Each importer classifies names with the bootstrap heuristic and merges them into `environments.toml` + `secrets.toml`. Existing entries are never overwritten.

```bash
# From a .env file — names only (default)
chibby import dotenv .env.production --env production

# From a .env file — also pull values (vars to environments.toml,
# secret values into the OS keychain)
chibby import dotenv .env.production --env production --with-values

# From Vercel (requires `vercel login` + `vercel link` in the project)
chibby import vercel --env production
chibby import vercel --env production --with-values   # runs `vercel env pull`

# From Railway (requires `railway login` + `railway link`)
chibby import railway --env production --with-values

# From Fly.io (names only — Fly's secrets API is write-only by design)
chibby import fly --env production
```

Each adapter fails with an actionable error message if the vendor CLI isn't installed or isn't authenticated.

### Export

Write resolved variables + secret values for an environment to a `.env` file. Useful for `dev` workflows that need a plain `.env` to point a local app at production-equivalent config without spelunking through keychain entries.

```bash
chibby export dotenv --env production --out .env.production.local
```

Output includes a `Do not commit` header. Variables come from the layered `environments.toml`; secret values are resolved from the keychain. Missing secrets are emitted as commented placeholders so the user knows what's still unset.

### Bootstrap

Scan a project for env/secret references and populate `.chibby/environments.toml` + `.chibby/secrets.toml` with the detected names. Values stay empty — set them with `chibby secrets set` / `chibby env vars set` afterwards.

```bash
# Show what would be detected without writing anything
chibby bootstrap --dry-run

# Apply (refuses if either config already exists)
chibby bootstrap

# Merge with existing configs — only adds newly-detected names
chibby bootstrap --merge

# Quieter output (skip the per-name table, just write)
chibby bootstrap --silent
```

Sources scanned: `.env*` files, `docker-compose*.yml`, `.github/workflows/*.yml`, and source code patterns (`process.env.X` in JS/TS, `os.getenv("X")` in Python, `env::var("X")` in Rust). Heuristic classifier sorts each detected name into a secret or variable based on word segments — `TOKEN`/`SECRET`/`KEY`/`PASSWORD`/`PAT`/`CREDENTIAL`/`PRIVATE`/`WEBHOOK` indicate secrets; `URL`/`HOST`/`PORT`/`PATH`/`DIR`/`NAME`/`REGION` indicate variables. Variable indicators win on collision (e.g. `PASSWORD_PATH` is a variable).

The GUI's Add Project wizard runs the same scan automatically, controlled by the `bootstrap_mode` app setting (`confirm` / `silent` / `off`, default `confirm`).

### Security Scans

```bash
# Scan for leaked secrets (gitleaks if installed; built-in regex fallback otherwise)
chibby scan secrets

# Scan dependencies for CVEs (auto-picks cargo audit / npm audit / pnpm audit / pip-audit)
chibby scan deps

# Lint commits against conventional-commits rules
chibby scan commits

# SAST — static analysis via semgrep
chibby scan sast

# Container image scan via trivy image (image refs from gates.toml, falls back to detected Dockerfiles)
chibby scan container

# Infrastructure-as-Code scan via trivy config (Dockerfile, docker-compose, k8s, terraform)
chibby scan iac

# License compliance — flags GPL/AGPL by default; configurable via gates.toml
chibby scan license

# Create a secret-scan baseline so existing findings stop blocking
chibby scan secrets --baseline
```

All gates load `.chibby/gates.toml` for allowlists, severity thresholds, and the
`container_images` list. Each scanner is detected at runtime — if the tool isn't
installed the gate returns a non-failing `"(missing)"` result with the install
command (`brew install gitleaks trivy semgrep`, `cargo install cargo-audit`, etc.).
Non-zero exit code on blocking findings — suitable for CI hooks.

Run all enabled gates at once from the desktop app's **Quality** tab, or wire
them into your pipeline by regenerating it after `gates.toml` exists — Chibby
auto-appends one `security-*` stage per enabled gate.

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
