# Security & Quality Gates

> **Quick start:** for a brand-new project, Chibby drops a default `.chibby/gates.toml` automatically. All gates start in `warn` mode and surface findings in the **Quality** tab without blocking runs. Once you've triaged the initial baseline, bump anything you care about to `block`.

---

Gates are project-scoped security and quality checks. Each gate wraps an external scanner (gitleaks, trivy, semgrep, etc.) and respects a single config file: `.chibby/gates.toml`. The same gates run from three surfaces:

- **CLI** — `chibby scan <gate>`
- **Desktop GUI** — Quality tab on Project Detail
- **Pipeline stages** — auto-appended by `chibby pipeline generate` when `gates.toml` exists

## The seven gates

| Gate | Maps to | Wraps | Catches |
|---|---|---|---|
| `secret_scanning` | SAST category 1 | `gitleaks` (or built-in regex fallback) | Hardcoded API keys, tokens, private keys in code/git history |
| `dependency_scanning` | SAST category 2 | `cargo audit` + `npm audit` / `pnpm audit` + `pip-audit` (auto-detected) | Known CVEs in direct + transitive deps |
| `commit_lint` | (style) | Built-in conventional-commits parser | Commit messages that don't follow `type(scope): subject` |
| `sast` | SAST category 3 | `semgrep --config=auto` | SQLi, XSS, command injection, insecure crypto, dangerous subprocess use, etc. |
| `container_scan` | SAST category 4 | `trivy image` | Vulnerabilities in OS packages + app deps inside container images |
| `iac_scan` | SAST category 5 | `trivy config` | Misconfigured Dockerfile / docker-compose / Kubernetes / Terraform / CloudFormation |
| `license_check` | SAST category 6 | `cargo-license` + `license-checker` | GPL/AGPL viral copyleft licenses in deps |

## Modes

Every gate has three modes:

| Mode | Behaviour |
|---|---|
| `block` | Findings fail the run (non-zero CLI exit, red banner in GUI) |
| `warn` | Findings are reported but the run still succeeds |
| `off` | Skip the gate entirely; doesn't appear in pipeline regen |

Default for newly-added projects: `warn` everywhere. Bump to `block` for the gates you've triaged.

## `gates.toml`

Stored at `.chibby/gates.toml`. The default file auto-created on project add looks like:

```toml
secret_scanning      = "warn"
dependency_scanning  = "warn"
commit_lint          = "warn"
sast                 = "warn"
container_scan       = "warn"
iac_scan             = "warn"
license_check        = "warn"

# Severity thresholds — block (or warn) on this level and above.
audit_severity_threshold     = "high"
sast_severity_threshold      = "high"
container_severity_threshold = "high"
iac_severity_threshold       = "high"

# Use baseline mode so existing test-fixture findings don't fail every run.
# Create with: chibby scan secrets --baseline
secret_scan_baseline = true

# Glob patterns excluded from secret scanning.
secret_scan_allowlist = [
  "**/__tests__/**", "**/__mocks__/**", "**/tests/**", "**/test/**",
  "**/*.test.ts", "**/*.spec.ts",
  "**/node_modules/**", "**/dist/**", "**/build/**",
  "**/.next/**", "**/.vercel/**",
]

# CVE IDs or package names ignored in dependency scanning.
audit_allowlist = []

# SAST rule IDs to ignore.
sast_allowlist = []

# Image refs to scan. When empty, the container gate falls back to Dockerfiles
# auto-discovered in the repo (top-level + one dir deep).
container_images = []

# SPDX license identifiers that fail license_check.
license_denylist = ["GPL-3.0", "GPL-2.0", "AGPL-3.0", "AGPL-1.0"]

# Package names exempt from license enforcement.
license_allowlist = []

# Commit lint rules.
commit_types = ["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"]
commit_max_subject_length = 100
commit_require_scope = false
```

Edit by hand or use the **Quality** tab in the desktop app (changes save back to the same file).

## Running gates

### From the CLI

```bash
# One at a time
chibby scan secrets       # gitleaks
chibby scan deps          # npm audit / cargo audit / pip-audit
chibby scan sast          # semgrep
chibby scan container     # trivy image
chibby scan iac           # trivy config
chibby scan license       # cargo-license / license-checker
chibby scan commits       # conventional-commits lint

# Create a secret-scan baseline (treats current findings as known/accepted)
chibby scan secrets --baseline
```

Each scanner is detected at runtime. Missing tools return a non-failing `"(missing)"` result with the install command — gates never block on "scanner not installed."

### From the desktop app

Open **Project Detail → Quality** tab. The `GatesCard` exposes:

- **Mode selector per gate** — `block` / `warn` / `off`
- **Severity threshold** inputs for dep audit, SAST, and container scan
- **Container images** textarea — one image ref per line; falls back to detected Dockerfiles when empty
- **Run all** button — runs every enabled gate and shows a passed/failed summary
- **Per-gate run buttons** — kick off one gate at a time with output rendered inline
- **Create secret-scan baseline** — same as `chibby scan secrets --baseline`

### From pipelines

When `.chibby/gates.toml` exists, `chibby pipeline generate` (or the GUI's **Regenerate** button) appends one `security-<gate>` stage per enabled gate to the produced pipeline:

```
security-secrets       chibby scan secrets
security-deps          chibby scan deps
security-sast          chibby scan sast
security-container     chibby scan container
security-iac           chibby scan iac
security-license       chibby scan license
security-commit-lint   chibby scan commits
```

Off-mode gates are skipped. Stages with `fail_fast = true` (the default) stop the run when a `block`-mode gate fails. Disable the auto-append for a project by either:

1. Deleting `.chibby/gates.toml`, OR
2. Setting every gate to `off`, OR
3. Removing the `security-*` stages by hand from `pipeline.toml`.

## Scanner install hints

| Scanner | macOS | Linux / other |
|---|---|---|
| gitleaks | `brew install gitleaks` | https://github.com/gitleaks/gitleaks |
| cargo-audit | `cargo install cargo-audit` | same |
| pip-audit | `pip install pip-audit` | same |
| semgrep | `brew install semgrep` or `pip install semgrep` | https://semgrep.dev/docs/getting-started/ |
| trivy | `brew install trivy` | https://aquasecurity.github.io/trivy/ |
| cargo-license | `cargo install cargo-license` | same |
| license-checker (npm) | `npm i -g license-checker` | same |

You don't have to install everything up front. Each gate gracefully reports its scanner as `"(missing)"` and prints the install hint when the tool isn't present.

## How findings appear

Every gate produces a structured result with:

- `passed: bool` — whether the gate met its threshold
- `findings: [...]` — concrete hits (file/line/rule/severity/message)
- `scanner: string` — which tool was used (or `"(missing)"`)
- `message: string` — human summary

In the CLI, findings render as a tree of warnings; non-zero exit on `passed: false`. In the GUI's Quality tab, results render inline below the per-gate Run button. In pipeline runs, the stage's stdout/stderr captures the same structured output.

## Auto-bootstrap on Add Project

When you add a new project (`chibby projects add` or the desktop Add Project wizard), Chibby seeds a default `gates.toml` so the Quality tab is populated from day one. The default never overwrites an existing `gates.toml`. You can disable this by adding the project, then editing the file (or deleting it).

The same hook runs alongside `auto_bootstrap_for_project` — see [Auto-bootstrap](env-secrets.md#auto-bootstrap).

## Recommendations panel

The right-side **Recommendations** panel on Project Detail flags two new "missing files" when they're absent:

- **`.chibby/gates.toml`** (High priority, Security) — without it, security stages won't appear in regenerated pipelines.
- **`.github/workflows/security.yml`** (High priority, Security) — paired with gates.toml, this gives you the same coverage in GitHub Actions for PRs and weekly schedules.

## See also

- [CLI reference — Security Scans section](cli-commands.md#security-scans)
- [User guide — Quality tab](../guides/user-guide.md#quality-tab)
- [Environments & Secrets](env-secrets.md) — different concept (deploy-time config), but `gates.toml` lives in the same `.chibby/` directory
