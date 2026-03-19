# Chibby Platform Knowledge

## Pipeline Format

Chibby uses TOML-based pipeline definitions with ordered stages.
Each stage specifies:
- `name`: human-readable stage name
- `commands`: list of shell commands to execute
- `working_dir`: optional working directory override
- `environment`: optional key-value environment variables
- `backend`: `local` (default) or `ssh` with connection details
- `stage_type`: `build`, `test`, `deploy`, `gate`, or `custom`

## Backends

- **Local**: commands run on the user's machine via the default shell.
- **SSH**: commands run on a remote host. Requires host, user, and optionally
  a key path. Connection is tested via preflight checks.

## Secrets

Managed via the OS keychain (macOS Keychain, Windows Credential Manager,
Linux Secret Service). Secrets are injected as environment variables at
stage execution time. Never logged in plaintext.

## Gates (Security & Quality)

- **Secret scanning**: detects leaked credentials in the codebase.
- **Dependency audit**: checks for known CVEs in project dependencies.
- **Commit linting**: validates commit message format.

Gates run before or after pipeline stages. Failed gates can block deployment.

## Artifacts

Pipeline stages can produce artifacts (binaries, packages, bundles).
Chibby collects, signs (macOS codesign, GPG), and manages retention.

## Run History

Every pipeline execution is recorded with full stage results, logs, timing,
and status. Users can retry failed runs or rollback to a previous successful run.
