# Environments & Secrets

> **Quick start:** for a brand-new project, run `chibby bootstrap` (or let the Add Project wizard do it). Chibby scans your repo and writes populated `environments.toml` + `secrets.toml` files. Then `chibby secrets set NAME --env production` to fill in values. See [Auto-bootstrap](#auto-bootstrap) below.

---


Chibby keeps deploy-time configuration in two places per project:

- **`.chibby/environments.toml`** — non-secret config (SSH host, environment variables). Committed to git.
- **`.chibby/secrets.toml`** — declared secret *references* (names + which environments they apply to). Committed to git.
- **OS keychain** — actual secret values. Never written to disk.

Plus a per-developer override file:

- **`.chibby/environments.local.toml`** — local overrides for individual contributors (their SSH host, dev-only variables). Automatically gitignored.

## Mental model

A run with `--env production` resolves variables in this order:

1. Start with the `[variables]` table of the `production` env from `environments.toml`.
2. Overlay the `production` block from `environments.local.toml` if it exists (per-developer overrides win).
3. Resolve every entry in `secrets.toml` against the OS keychain and merge those in.
4. The resulting `KEY=VALUE` map is exported to every stage — locally via `Command::envs(...)`, and over SSH via `export K='V' && ...`.

## File formats

### environments.toml (committed)

```toml
[[environments]]
name = "production"
ssh_host = "deploy@prod.example.com"
ssh_port = 22

[environments.variables]
API_URL       = "https://api.example.com"
LOG_LEVEL     = "info"
DEPLOY_DIR    = "/srv/myapp"
```

### environments.local.toml (gitignored, per-developer)

Only fields you want to override need to be present. Missing fields fall back to the committed file.

```toml
[[environments]]
name = "production"

# Override just the host so your laptop talks to a bastion
ssh_host = "user@bastion.example.com"

# Add a dev-only variable
[environments.variables]
DEBUG = "true"
```

### secrets.toml (committed, names only)

```toml
[[secrets]]
name = "DEPLOY_KEY"
environments = ["production"]   # omit to apply to all environments

[[secrets]]
name = "SLACK_WEBHOOK"
environments = []               # empty = all envs
```

Values never go in this file — set them with `chibby secrets set NAME --env ENV` or via the desktop Secrets panel.

## Day-to-day workflow

### Adding a new environment

```bash
chibby env add staging --ssh-host deploy@staging.example.com
chibby env vars set staging API_URL https://staging.api.example.com
chibby env vars set staging LOG_LEVEL debug
```

### Adding a new secret

```bash
chibby secrets add STRIPE_KEY --env production --env staging
chibby secrets set STRIPE_KEY --env production    # prompts securely
chibby secrets set STRIPE_KEY --env staging
```

### Cloning a repo as a new team member

After cloning, `environments.toml` and `secrets.toml` are present but the keychain on your machine is empty.

```bash
chibby doctor                          # see what's missing
chibby secrets set DEPLOY_KEY  --env production
chibby secrets set STRIPE_KEY  --env production
chibby secrets set SLACK_WEBHOOK --env production
chibby doctor                          # confirm clean
```

If your local network needs a different SSH host than the committed value:

```bash
chibby env vars set production SSH_HOST_OVERRIDE bastion.local --local
```

The `--local` flag writes to `environments.local.toml` and adds it to `.gitignore`.

### Rotating a secret

```bash
chibby secrets rotate STRIPE_KEY --env production
```

This re-prompts for the value and overwrites the existing keychain entry. Other developers' keychains are not affected — they rotate independently.

### Comparing two environments

```bash
chibby env diff production staging
```

`+` lines exist only in the destination, `-` lines only in the source, `~` lines have a different value. Identical entries are summarised as "identical" — the output stays compact even on large configs.

### Running a deploy that uses everything

```bash
chibby run --env production
```

Stages backed by `local` get vars + secrets through `Command::envs(...)`. Stages backed by `ssh` get them via `export KEY='val'` prefixes on the remote shell.

## Auto-bootstrap

For new projects, Chibby can populate `environments.toml` and `secrets.toml` automatically by scanning the repo for env-variable references.

### Sources scanned

| Source | What's extracted |
| ------ | ---------------- |
| `.env*` files | Keys only (values discarded) |
| `docker-compose*.yml` | `${VAR}`, `${VAR:-default}` interpolations |
| `.github/workflows/*.yml` | `${{ secrets.X }}` references |
| `*.js`, `*.ts`, `*.jsx`, `*.tsx`, `*.mjs`, `*.cjs` | `process.env.X`, `process.env["X"]` |
| `*.py` | `os.getenv("X")`, `os.environ["X"]`, `os.environ.get("X")` |
| `*.rs` | `env::var("X")`, `std::env::var("X")`, `env::var_os("X")` |

Skip directories: `node_modules`, `target`, `venv`, `.venv`, `__pycache__`, `dist`, `build`, `.git`, `.chibby`, `.next`, `.nuxt`, `coverage`.

### Classification

Each detected name is classified as **secret** or **variable** using name-segment heuristics:

- **Secret indicators** (any segment): `TOKEN`, `SECRET`, `PASSWORD`, `PASSWD`, `PAT`, `CREDENTIAL`, `PRIVATE`, `APIKEY`, `SIGNING`, `WEBHOOK`, `DSN`, `BEARER`, or a `KEY` segment.
- **Variable indicators** (win over secret indicators when both present): `URL`, `HOST`, `HOSTNAME`, `PORT`, `PATH`, `DIR`, `DIRECTORY`, `NAME`, `MODE`, `ENV`, `REGION`, `VERSION`, `STAGE`, `TIMEOUT`.
- **Default for unknown names**: variable. Conservative bias — a misclassified non-secret in `environments.toml` is recoverable; a non-secret in the keychain is friction.

False-positive examples handled correctly: `MONKEY` (not KEY), `KEYBOARD_LAYOUT` (not KEY), `PASSWORD_PATH` (PATH wins → variable).

### Environment inference

The scan also suggests environment names based on the files it finds:

- `.env.production` → `production`
- `.env.staging` → `staging`
- `docker-compose.prod.yml` → `production`
- `docker-compose.staging.yml` → `staging`

If nothing suggests an environment, `production` is the default.

### App setting: `bootstrap_mode`

Controls what the GUI does when you add a project:

| Mode | Behaviour |
| ---- | --------- |
| `confirm` (default) | Scan and show a review modal — you check/uncheck names and classifications before writing |
| `silent` | Scan and write configs immediately, no review |
| `off` | Skip the scan entirely |

Set it from the desktop Settings panel, or edit `<data_dir>/settings.toml`:

```toml
bootstrap_mode = "silent"
```

### Apply modes (CLI)

| Flag | Behaviour |
| ---- | --------- |
| _(default)_ | Refuses to write if `environments.toml` or `secrets.toml` already exists |
| `--merge` | Appends only newly-detected names; never modifies existing entries |
| `--dry-run` | Prints what would be written without touching the filesystem |
| `--silent` | Skip the per-name preview table (still writes) |

## Importers

Adapters for pulling references (and optionally values) from external sources.

| Source | Names | Values | Notes |
| ------ | ----- | ------ | ----- |
| `dotenv` | ✓ | ✓ | Parses `KEY=VALUE`, supports quoted values + `export` prefix |
| `vercel` | ✓ | ✓ | Names via `vercel env ls --json`; values via `vercel env pull`. Requires `vercel login` + `vercel link`. |
| `railway` | ✓ | ✓ | Single call to `railway variables --json`. Requires `railway login` + `railway link`. |
| `fly` | ✓ | ✗ | Names from `flyctl secrets list --json`. Fly's secrets API is write-only by design. |

All importers reuse the bootstrap classifier — a name detected as `STRIPE_SECRET` will land in `secrets.toml` regardless of which adapter found it.

```bash
# Pull a .env file end-to-end (variables to environments.toml,
# secret values into the keychain)
chibby import dotenv .env.production --env production --with-values

# Bring Vercel's production env into Chibby
chibby import vercel --env production --with-values

# Round-trip — re-emit a .env file from Chibby's configs
chibby export dotenv --env production --out .env.production.local
```

## Safety features

### Audit metadata

Every set/delete on a secret value is recorded under `<chibby_data_dir>/secret_audit/<repo_hash>.json` with:

- `last_set` / `last_deleted` timestamps (UTC)
- `set_count` / `delete_count`
- `last_provenance` — `cli`, `gui`, `import:vercel`, `import:dotenv`, etc.

Inspect via `chibby audit list` or `chibby audit show NAME --env ENV`. Useful for "when did I last rotate this?" and "is this secret still in use anywhere?" questions during incident response.

The audit file lives in the user's Chibby data dir, not the repo — so it never gets accidentally committed and follows the user across project clones.

### Leak scanning

`environments.toml` is *only* for non-secret config. If a token-shaped value lands in a variable by accident, Chibby flags it:

- On every `save_environments` call, a warning is logged.
- `chibby env scan-leaks` runs an explicit scan and exits non-zero when anything matches.
- Patterns covered: GitHub PATs, GitHub fine-grained PATs, GitLab PATs, OpenAI / Anthropic API keys, Slack tokens, Stripe keys, SendGrid keys, AWS access key IDs, Twilio keys, private-key blocks, database URLs with embedded credentials.
- Previews are redacted (`ghp_…(40 chars)`) — the suspect value is never echoed verbatim in logs or output.

A separate full-repo gitleaks-backed scan is available via `chibby scan secrets` (configured in `.chibby/gates.toml`) and includes `.chibby/*.toml` files by default.

## How keychain storage works

| OS | Backend |
| -- | ------- |
| macOS | Keychain (`security` API) |
| Linux | Secret Service (libsecret / GNOME Keyring / KWallet) |
| Windows | Credential Manager |

The keychain account key is `<project_path>|<environment>|<secret_name>`, percent-encoded so segments can't collide. Two projects on the same machine using the same secret name in the same environment do not conflict.

## Doctor

Run `chibby doctor` to validate everything end-to-end:

- Config files present (`pipeline.toml`, `environments.toml`, `secrets.toml`)
- Each environment's SSH host is reachable (`ssh -o BatchMode=yes`)
- Every declared secret has a value in the keychain for every environment it applies to

Non-zero exit code on any failure — wire it into CI before `chibby run --env production` to fail fast on a misconfigured deploy machine.

## Desktop GUI

Everything described above is also reachable from the desktop app. Open a project, switch to the **Environments** tab, and you get the same surface area as the CLI.

### Bootstrap & Import bar

A three-button toolbar at the top of the tab:

| Button | Action |
|--------|--------|
| **Bootstrap** | Opens the wizard — runs `scan_bootstrap`, lists every detected name with its classification and source files, then applies in Safe (default) or Merge mode. |
| **Import…** | Opens the importer modal for `.env`, Vercel, Railway, or Fly.io. The modal probes the vendor CLI first and reports presence/absence before you run. |
| **Export .env** | Opens a save dialog; writes the resolved variables and secret values for the chosen environment to a flat `.env` file. |

The Bootstrap wizard also runs automatically right after **Add Project** when `bootstrap_mode = "confirm"` (the default). Set the mode from Settings → About.

### Environments card

The editor has three modes:

| Mode | What it edits |
|------|---------------|
| **Committed** | `.chibby/environments.toml` — the file that ships with the repo |
| **Local overrides** | `.chibby/environments.local.toml` — per-developer file (auto-gitignored on save) |
| **Layered (read-only)** | The merged view that a run with `--env <name>` actually sees |

A red **Leak warning banner** appears above the editor whenever `scan_environments_for_leaks` finds token-shaped strings inside `environments.toml`. Each hit lists env, variable, rule name, and a redacted preview. The scan re-runs on every save.

### Secrets card

Per-environment Set / Delete with one addition: a **clock icon** on every row opens the **Secret audit modal** with the same data CLI's `chibby audit show` displays — `last_set`, `last_deleted`, `set_count`, `delete_count`, and `last_provenance`.

### Bootstrap mode in Settings

Settings → About has a **Bootstrap mode (new projects)** selector that maps directly to the values described in [App setting: `bootstrap_mode`](#app-setting-bootstrap_mode). `confirm` / `silent` / `off`.

## See also

- [CLI reference](cli-commands.md) — full subcommand list with examples
- [Templates](templates.md) — using secret refs in built-in deploy templates
- [User guide — Environments tab](../guides/user-guide.md#environments-tab) — step-by-step walkthroughs of the GUI
