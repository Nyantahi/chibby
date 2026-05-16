# Environments & Secrets Feature Epic

> **Status:** Complete — four iterations shipped as stacked PRs (#40, #41, #42, #43).
> **Timeline:** May 2026.
> **Owner:** @Nyantahi
> **See also:** [`features/env-secrets.md`](../features/env-secrets.md), [`features/cli-commands.md`](../features/cli-commands.md)

---

## Why this work happened

Before this epic, Chibby's env/secret story had three large gaps:

1. **CLI was fake.** Every `chibby env` / `chibby secrets` subcommand was a stub that printed hardcoded data (`DEPLOY_KEY`, `staging.example.com`) and never touched the engine. The `chibby-cli` binary didn't even compile against the current `executor::run_pipeline` signature — two call sites were missing the 8th argument.
2. **Empty by default.** A freshly-added project got a generated `pipeline.toml` but empty `environments.toml` and `secrets.toml`. New users had to know what env-var names mattered for their stack and add them by hand.
3. **No safety net.** No leak detection on `environments.toml` values, no per-secret audit history, no way to compare two environments.

For an opensource dev tool the "open it and it works" moment matters as much as feature count. This epic addressed all three gaps, while keeping the public API surface small and adding 51 unit tests (103 → 154).

---

## Original plan

Brainstormed across themes; the user picked PR-per-iteration and a four-stage sequence:

### Themes considered
1. **Auto-bootstrap on project add** — scan repo for env/secret signals, write populated configs
2. **CLI parity with the real engine** — replace stubs, add new subcommands
3. **Importers** — dotenv + bundled PaaS adapters (Vercel/Railway/Fly)
4. **Team workflow** — `environments.local.toml` overlay for per-developer overrides
5. **Safety + observability** — audit metadata, leak detection, env diff

### Trade-offs flagged up front
- Auto-bootstrap can mis-classify (e.g. `DATABASE_URL`). Mitigation: conservative bias toward "variable" + user can review/reclassify.
- `environments.local.toml` adds a new convention. Worth it — matches `direnv` / `docker-compose.override.yml` precedent.
- Bundled PaaS importers carry maintenance cost as vendor CLIs change. Accepted because they're opt-in (no install = no breakage).
- CLI/GUI feature parity is a permanent tax. Worth it for opensource credibility.

### Three scoping decisions the user made
1. **Bootstrap aggressiveness** — both modes (silent + confirm), with a `bootstrap_mode` setting.
2. **`environments.local.toml` layering** — adopt now.
3. **PaaS importers** — bundle them into the binary (not optional opt-in hooks).
4. **Iteration 4 scope** — keep in (full feature, not "ship 1–3 and polish later").
5. **PR strategy** — one PR per iteration (stacked, reviewable independently).

---

## What shipped, iteration by iteration

### Iteration 1 — Real CLI handlers + layered configs ([PR #40](https://github.com/Nyantahi/chibby/pull/40))

**Goal:** Every `chibby env*` / `chibby secrets*` command works. `environments.local.toml` overlays the committed file.

**Changes:**
- `src-tauri/src/bin/chibby.rs` — replaced every stub handler with real engine calls. Added `--environment` / `-e` flag to all secret commands (the engine was always per-env; only the CLI ignored it). New subcommands: `secrets list/rotate`, `env add/edit/copy`, `env vars set/get/list/delete`, `doctor`.
- `src-tauri/src/engine/pipeline.rs` — extended with `load_environments_layered`, `merge_environments` (pure function), plus granular update helpers (`add_environment`, `set_env_variable`, `add_secret_ref`, etc.) so the CLI doesn't have to round-trip whole files.
- `ensure_gitignore_entries` — idempotent, marker-based writer that adds `.chibby/environments.local.toml` and `.chibby/secrets.local.toml` to `.gitignore` automatically.
- `src-tauri/src/engine/run_support.rs` — switched to `load_environments_layered` so deploys see the merged view.
- `src-tauri/src/commands/env_commands.rs` — exposed `load_environments_layered`, `load_environments_local`, `save_environments_local` as Tauri commands.
- Fixed a latent compile break: two stale `executor::run_pipeline` calls in `bin/chibby.rs` that prevented the CLI binary from building at all.

**Doctor command:** end-to-end check (config files present, SSH reachable, secrets resolved in keychain), non-zero exit on any failure — wireable into CI before `chibby run --env production`.

**Layered merge rule (`merge_environments`):**
- By env name. Local-`Some` wins on `ssh_host` / `ssh_port`. `variables` deep-merged with local winning on collision.
- Envs only in local are appended.

**Tests added:** 11. Coverage: merge semantics, granular helpers reject duplicates, gitignore idempotency.
**Test total:** 103 → 114.

### Iteration 2 — Auto-bootstrap on Add Project ([PR #41](https://github.com/Nyantahi/chibby/pull/41))

**Goal:** A newly-added project gets populated `environments.toml` + `secrets.toml` automatically. Values stay empty — user fills them via `chibby secrets set` or the GUI.

**New module:** `src-tauri/src/engine/bootstrap.rs`

| Source | What's extracted |
| ------ | ---------------- |
| `.env*` files | Keys only (values discarded) |
| `docker-compose*.yml` | `${VAR}` / `${VAR:-default}` / `$VAR` interpolations |
| `.github/workflows/*.yml` | `${{ secrets.X }}` references |
| `*.{js,ts,jsx,tsx,mjs,cjs}` | `process.env.X`, `process.env["X"]` |
| `*.py` | `os.getenv("X")`, `os.environ["X"]`, `os.environ.get("X")` |
| `*.rs` | `env::var("X")`, `std::env::var("X")`, `env::var_os("X")` |

**Classifier — name-segment heuristic with conservative bias:**
- Secret indicators (segment match): `TOKEN`, `SECRET`, `PASSWORD`, `PAT`, `CREDENTIAL`, `PRIVATE`, `APIKEY`, `SIGNING`, `WEBHOOK`, `DSN`, `BEARER`, standalone `KEY`.
- Variable indicators (win on collision): `URL`, `HOST`, `PORT`, `PATH`, `DIR`, `NAME`, `MODE`, `ENV`, `REGION`, `VERSION`, `STAGE`, `TIMEOUT`.
- Unknown defaults to variable. A misclassified non-secret in `environments.toml` is easily moved; a non-secret in the keychain is friction.
- Handles false positives: `MONKEY` and `KEYBOARD_LAYOUT` correctly stay variables (segment-based, not substring); `PASSWORD_PATH` is a variable because `PATH` wins.

**Environment inference:** `.env.production` → `production`, `docker-compose.prod.yml` → `production`, `docker-compose.staging.yml` → `staging`. Default `production`.

**Apply modes:**
- **Safe:** refuses if `environments.toml` or `secrets.toml` already exists.
- **Merge:** appends only newly-detected names; never modifies existing entries.

**New CLI:** `chibby bootstrap [--dry-run] [--merge] [--silent]`.

**GUI integration:**
- New `AppSettings.bootstrap_mode` (`confirm` | `silent` | `off`, default `confirm`).
- Three Tauri commands: `scan_bootstrap`, `apply_bootstrap`, `auto_bootstrap_for_project`.
- The frontend AddProject wizard can call `auto_bootstrap_for_project` after `add_project` resolves.

**Tests added:** 15. Coverage: each per-language parser, false-positive classifier cases, env inference, end-to-end scan+apply with Merge preserving existing entries.
**Test total:** 114 → 129.

### Iteration 3 — Importers + dotenv export ([PR #42](https://github.com/Nyantahi/chibby/pull/42))

**Goal:** Bring existing env into Chibby from `.env`, Vercel, Railway, or Fly.io. Round-trip via `chibby export dotenv` for local-dev workflows.

**New module:** `src-tauri/src/engine/importers/`

```rust
pub trait Importer {
    fn name(&self) -> &'static str;
    fn detect_cli(&self) -> Result<()>;  // actionable install hint
    fn run(&self, ctx: &ImportContext) -> Result<ImportReport>;
}
```

| Adapter | Names | Values | How |
| ------- | ----- | ------ | --- |
| `dotenv` | ✓ | ✓ | Parses `KEY=VALUE` with quote/escape/`export` handling |
| `vercel` | ✓ | ✓ | `vercel env ls --json` + `vercel env pull` to tempfile (re-uses dotenv parser) |
| `railway` | ✓ | ✓ | Single `railway variables --json` call |
| `fly` | ✓ | ✗ | `flyctl secrets list --json` — values not available (Fly is write-only by design) |

**`apply_report()` — always Merge mode by design.** Existing entries are never overwritten. A hand-tuned `[environments.variables]` table survives untouched. `ApplyOptions.persist_secret_values` lets non-interactive callers skip keychain writes.

**`export_dotenv()` — symmetric round-trip.** Variables from layered `environments.toml`, secret values from keychain. Missing secrets emit commented placeholders (`# DEPLOY_TOKEN= (missing — run \`chibby secrets set\`)`). Values with whitespace / `#` / quotes are properly quoted and escaped.

**New CLI:**
```bash
chibby import dotenv|vercel|railway|fly --env <name> [--with-values]
chibby export dotenv --env <name> --out <path>
```

**Tauri commands:** `run_importer`, `importer_cli_status`, `export_dotenv`.

**Cargo:** `tempfile` promoted from dev-dependency to a regular dependency (Vercel adapter uses it for `vercel env pull` target file).

**Tests added:** 10. Coverage: dotenv parser quirks (quoted/escaped values, `export` prefix, comments), classifier dispatch, apply preserves existing values, export handles missing keychain entries.
**Test total:** 129 → 139.

### Iteration 4 — Safety & polish ([PR #43](https://github.com/Nyantahi/chibby/pull/43))

**Goal:** Per-secret audit history, leak scanning of `environments.toml` values, env-to-env diff.

**New module:** `src-tauri/src/engine/secret_audit.rs`
- Per-project, per-secret lifecycle: `last_set` / `last_deleted` timestamps, `set_count` / `delete_count`, `last_provenance`.
- `Provenance` enum: `Cli`, `Gui`, `Import { adapter }`, `Export`, `Unknown`.
- Stored at `<chibby_data_dir>/secret_audit/<sha256[:16]>.json` — follows the user's Chibby install, not the repo. Owner-only on Unix (`0600`).
- Failure-quiet helpers (`record_set_quietly`, `record_delete_quietly`) — audit is observability, never gating.

**New module:** `src-tauri/src/engine/leak_scanner.rs`
- In-process regex set covering GitHub PATs (classic + fine-grained), GitLab PATs, OpenAI / Anthropic keys, Slack tokens, Stripe keys, SendGrid, AWS access key IDs, Twilio, private-key blocks, database URLs with embedded credentials.
- Patterns compile once via `std::sync::OnceLock` (no extra dep).
- `redact()` shapes previews as `ghp_…(40 chars)` — the actual secret never appears in any output, log, or error.

**Wiring:**
- `pipeline::save_environments` runs the leak scanner after serialise and logs a `warn!` if any variable value matches. Save still succeeds — gating belongs in `gates.rs`.
- `pipeline::scan_environments_for_leaks(repo)` for explicit queries.
- Audit hooks added to GUI (`Provenance::Gui`), CLI (`Provenance::Cli`), and importer (`Provenance::Import { adapter }`) edges.

**New CLI:**
```bash
chibby env diff <a> <b>         # ± / ~ markers for variable + secret deltas
chibby env scan-leaks            # explicit leak scan, non-zero on hits
chibby audit list                # per-project secret history summary
chibby audit show NAME --env ENV # one secret's full snapshot
```

**Push-protection gotcha caught:** the leak-scanner tests used deliberately fake fixtures (e.g. `xoxb-1234567890-...XXXX`) that were structured enough to trigger GitHub's secret-scanning push protection. Resolution: assemble test strings at runtime by concatenating small literals — coverage identical, source contains no contiguous credential-shaped strings. Documented in the source so future contributors don't repeat it.

**Tests added:** 14. Coverage: each leak-rule, redaction quality, sort order, audit provenance for all sources, audit isolation across projects.
**Test total:** 139 → 154.

---

## Final feature surface

### CLI commands

```
chibby env list/show/add/remove/edit/copy/test
chibby env vars set/get/list/delete [--local]
chibby env diff <a> <b>
chibby env scan-leaks

chibby secrets list/add/remove/set/rotate/delete/status

chibby bootstrap [--dry-run|--merge|--silent]
chibby import dotenv|vercel|railway|fly --env <name> [--with-values]
chibby export dotenv --env <name> --out <path>

chibby audit list
chibby audit show <name> --env <env>

chibby doctor
```

### File layout in a project

```
.chibby/
├── pipeline.toml                  # CI/build stages (existing)
├── environments.toml              # committed: ssh_host + non-secret variables
├── environments.local.toml        # gitignored: per-developer overrides
└── secrets.toml                   # committed: secret references (names only)

# Plus, in the user's Chibby data dir (not the repo):
<chibby_data_dir>/secret_audit/<sha256[:16]>.json   # per-secret lifecycle
```

### Tauri commands exposed for the GUI

`load_environments`, `load_environments_layered`, `load_environments_local`, `save_environments`, `save_environments_local`, `load_secrets_config`, `save_secrets_config`, `set_secret`, `delete_secret`, `check_secrets_status`, `test_ssh_connection`, `scan_bootstrap`, `apply_bootstrap`, `auto_bootstrap_for_project`, `run_importer`, `importer_cli_status`, `export_dotenv`, `get_secret_audit`, `scan_environments_for_leaks`, `run_preflight`.

### App-level settings

`AppSettings.bootstrap_mode: "confirm" | "silent" | "off"` (default `confirm`).

---

## What's deferred

All deferred items are pure frontend work — backend support is in place.

- **AddProject review modal** — calls existing `auto_bootstrap_for_project`. Shows detected names grouped by source with checkboxes + reclassify dropdowns before writing.
- **"Import from..." menu** in Secrets/Environments panels — calls existing `run_importer`.
- **Audit display** in EnvironmentEditor — calls existing `get_secret_audit`.
- **Reveal gate** in SecretsManager — masked-by-default secret display with OS re-auth (Touch ID / Windows Hello via Tauri webview APIs).

---

## Lessons learned

- **CLI tests against the engine catch fakery.** Replacing stubs surfaced a latent compile break that had been on `main` for an unknown length of time. Integration smoke tests that actually call the engine (not just `--help` output) would have caught this earlier.
- **Conservative classification beats clever classification.** The bootstrap classifier defaults ambiguous names to "variable." Easier to recover from than the alternative (a non-secret rotted in keychain).
- **Push protection scans every commit, not just the tip.** If you commit a test fixture that looks real, then add a follow-up fix, the offending commit is still in the branch history. Squash with `git reset --soft` to clean it up (or write fixtures so they never trip protection in the first place).
- **OnceLock over Lazy.** Avoid pulling in `once_cell` when `std::sync::OnceLock` (Rust 1.70+) does the same job for static regex tables.

---

## Stack of PRs

| PR | Iteration | Lines added | Tests added |
| -- | --------- | ----------- | ----------- |
| [#40](https://github.com/Nyantahi/chibby/pull/40) | Iter 1 — CLI parity + layered configs | +1,266 | +11 |
| [#41](https://github.com/Nyantahi/chibby/pull/41) | Iter 2 — auto-bootstrap on Add Project | +1,164 | +15 |
| [#42](https://github.com/Nyantahi/chibby/pull/42) | Iter 3 — dotenv + Vercel/Railway/Fly importers + export | +1,248 | +10 |
| [#43](https://github.com/Nyantahi/chibby/pull/43) | Iter 4 — secret audit + leak scanning + env diff | +873 | +14 |

103 → 154 lib tests across the stack, all green.
