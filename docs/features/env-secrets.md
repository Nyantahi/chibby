# Environments & Secrets

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

### Running a deploy that uses everything

```bash
chibby run --env production
```

Stages backed by `local` get vars + secrets through `Command::envs(...)`. Stages backed by `ssh` get them via `export KEY='val'` prefixes on the remote shell.

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

## See also

- [CLI reference](cli-commands.md) — full subcommand list with examples
- [Templates](templates.md) — using secret refs in built-in deploy templates
