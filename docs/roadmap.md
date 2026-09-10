# Roadmap & Gap Analysis

Where Chibby v0.3.1 stands against what a CI/CD tool is expected to do, ranked by
urgency.

## Method

Two inputs:

1. **Codebase audit** — reading the actual Rust engine, Tauri commands, CLI, and React
   frontend rather than the feature list. Every gap below cites the file that proves it.
2. **External research** — 2026 surveys and practitioner write-ups on where CI/CD
   pipelines hurt, used to sanity-check that the gaps matter to real users and aren't
   just missing checkboxes.

The internal phased build plan already backlogs several of the obvious gaps: cron
scheduling, incoming webhooks, multi-host environments, a `parallel_group` field on
stages, a GUI dry-run, and an insights dashboard slated as "Phase 7.5". Those are known
work, and this document says so where it applies.

What this document adds is the **newly identified** set — and several of those are not
missing features at all. They are defects: behaviour that is broken, inconsistent, or
silently unsafe today, in code that already ships.

## Tier 0 — defect-grade

Broken today. None of these are "not built yet"; each is a hole in something already
shipping.

Rows marked **FIXED** were closed by the work described under [Shipped](#shipped); they are
kept here because the evidence explains *why* they mattered. The rest are still open.

| Gap                                                  | Evidence                                                                                                                                                                                                                                        | Impact                                                                                           |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| **No per-stage timeout** — FIXED                             | `src-tauri/src/engine/executor.rs` bounds nothing. Only the AI agent path has a limit (`AGENT_COMMAND_TIMEOUT_SECS`, `agent/command_exec.rs`)                                                                                                   | A hung command hangs the run forever, GUI included. Blocks unattended execution entirely         |
| **`PipelineRun.branch` / `.commit` are dead fields** — FIXED | Set to `None` in `engine/models/run.rs::new_with_id()` and never assigned anywhere. `DeploymentRecord` copies them straight through (`persistence.rs:233`), so it inherits the hole. `engine/git.rs::info()` works fine — nothing calls it here | Run history and deployment records carry no git provenance. "What shipped where" is unanswerable |
| **Secrets not masked in pipeline logs** — FIXED              | Redaction exists only at `agent/context.rs::sanitize_log_line`. The executor streams raw stdout/stderr, persisted inline in `runs/<uuid>.json`                                                                                                  | Any command that echoes a secret writes it to disk in plaintext, permanently                     |
| **GUI runs skip preflight, CLI runs don't** — FIXED          | `commands/run_commands.rs::run_pipeline` never calls preflight; `bin/chibby/runs.rs` does by default                                                                                                                                            | Same pipeline, two behaviours, depending on which surface you launched it from                   |
| **OpenAI provider can't drive the agent**            | `src-tauri/src/ai/provider.rs:244` — `"Tool use is not supported by the '{}' provider"`                                                                                                                                                         | Provider selection is a headline README feature, but Advise/Act effectively require Anthropic    |
| **Two dead components ship in the bundle**           | `frontend/components/DashboardOverview.tsx` (234 lines) and `PipelineGenerator.tsx` (159 lines) — zero references anywhere in the tree                                                                                                          | Working UI nobody can reach                                                                      |

## Tier 1 — it isn't CI without this

### 1. No automatic triggers at all

A manual GUI click or `chibby run` are the only entry points. No cron, no file watch, no
git hook. Related symptom: `chibby cancel` is a no-op that prints "press Ctrl-C in the
`chibby run` terminal" (`bin/chibby/runs.rs:305`), because there is no daemon to signal.

Research consistently centres CI/CD complaints on the feedback loop — change, push,
wait, debug, repeat — with pipeline wait time and slow iteration the recurring theme
([CI/CD pipeline statistics and trends,
2026](https://www.incredibuild.com/blog/cicd-pipeline-statistics-trends-2026); [Things I
don't like about GitHub
Actions](https://medium.com/@elkourchimohammed/things-i-dont-like-about-github-actions-8862cf808735)).
A local-first tool is the natural answer to that loop. Chibby currently doesn't take the
shot.

Cron and file-watch triggers are backlogged in the phased build plan.

### 2. Stage model too thin for real pipelines

`Stage` (`engine/models/pipeline.rs:23`) is `{name, commands, backend, working_dir,
fail_fast, health_check}`. That's it. No conditionals or branch filters, no per-stage
env, no retry/backoff, no parallelism, no DAG, no matrix builds, no artifact passing
between stages, no caching, no approval gates. `fail_fast` is the only control flow the
model has.

A `parallel_group` field is backlogged; the rest is not.

### 3. Health checks detect but don't act

`HealthCheck` runs post-stage and sets `health_check_passed` on the stage result. The run
then just fails. Rollback exists (`engine/run_support.rs`) but is human-initiated after
the fact.

Standard practice is a smoke test gating promotion, wired to automatic rollback on
failure ([Integrating smoke testing into your CI/CD
pipeline](https://www.harness.io/harness-devops-academy/integrating-smoke-testing-into-your-ci-cd-pipeline-what-devops-needs-to-know)).
Chibby has both halves built and no wiring between them.

### 4. No insight beyond four tiles

Success rate is today-only. There are no duration trends, no failure clustering, no
flaky-stage detection, no environment/version matrix, no log search, no run diffing, and
no DORA metrics.

An insights dashboard is backlogged as a later phase.

## Tier 2 — real, lower urgency for a solo/small-team tool

| Gap                                 | Detail                                                                                                                                                                                                                                                                                                                                                                                                      |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Deploy strategies**               | No release-dir/symlink deploys, no blue-green, no canary, no multi-host fan-out — `Environment` (`engine/models/environment.rs:9`) holds a single `ssh_host`. Multi-host environments are backlogged                                                                                                                                                                                                        |
| **Database migrations**             | Widely cited as the single most common failure point for teams automating deploys ([Database migrations in CI/CD pipelines](https://khimananda.com/blog/database-migrations-in-ci-cd-pipelines)). Chibby has no notion of a gated, separately-rollback-able migration stage                                                                                                                                 |
| **No container execution backend**  | `Backend` is `Local \| Ssh` only. Docker appears as _content_ (compose commands over SSH), but nothing runs _inside_ a container, so Chibby can't offer environment parity — the "works on my machine" problem it is otherwise well positioned to solve ([Why "it worked on my machine" still happens in 2026](https://www.freecodecamp.org/news/why-it-worked-on-my-machine-still-happens-in-2026/))       |
| **Supply-chain attestation**        | Seven scanning gates, but no SBOM generation and no SLSA/cosign provenance. Regulatory pressure (US EO 14028, EU Cyber Resilience Act) is making this table stakes ([2026 guide to software supply chain security](https://cloudsmith.com/blog/the-2026-guide-to-software-supply-chain-security-from-static-sboms-to-agentic-governance)). Chibby already signs artifacts, so the primitives are half there |
| **Run history doesn't scale**       | `persistence::load_runs()` deserializes _every_ run file — each with full stdout/stderr inline — then sorts, on every history query. `load_runs_for_project()` filters afterwards. Fine at 50 runs, not 5,000                                                                                                                                                                                               |
| **No audit-log viewer**             | `engine/audit.rs` writes an append-only `audit.log` covering secret changes, runs, and AI interactions. No UI surfaces it                                                                                                                                                                                                                                                                                   |
| **Credential/cert expiry warnings** | Apple signing certs and update signing keys expire. Nothing warns before they do                                                                                                                                                                                                                                                                                                                            |

Preview environments per branch are a related adjacent expectation
([Preview environment platforms](https://northflank.com/blog/preview-environment-platforms)),
but they presuppose the trigger and container work above.

## Out of scope

These are deliberate product decisions, not oversights:

- Inbound webhook receiver
- Remote runners / agents
- Multi-user and RBAC
- Shared run history
- Forge status reporting (PR checks)

Each requires a listener, a server, or an identity model. All three contradict the
local-first positioning the README's comparison table sells — offline, no cloud, no
account, runs on your machine. Adding any of them turns Chibby into a smaller version of
the tools it exists to avoid.

## Shipped

Three items, in dependency order. The ordering was forced: timeouts had to exist before
unattended runs were safe (without a bound, one hung command blocks everything queued
behind it with nobody watching), and rollback had to be automatic before deploys started
happening unattended — otherwise a trigger just means unattended breakage.

### 1. Stage execution hardening

`Stage` gains `timeout_secs`, `retry` (attempts / delay / fixed-or-exponential backoff),
`when` (branch and environment globs, with `_not` exclusions), and a per-stage `env`
overlay. A timed-out stage records the new `StageStatus::TimedOut` and is treated as a
failure everywhere. Skipped stages record *why*.

Closes four Tier-0 rows: runs now carry real `branch`/`commit`; secret values are redacted
from log output **at ingest**, so `runs/<uuid>.json` never holds plaintext; and the GUI runs
preflight like the CLI always did.

### 2. Auto-rollback on failed health check

`RollbackPolicy` on a stage or pipeline: `last_good` replays the last known-good
deployment's snapshot, `commands` runs the stage's own `rollback_commands` (`kubectl
rollout undo`, `flyctl releases rollback`, …).

"Last known good" deliberately does **not** mean "last successful run" — it requires that
the deploy stage actually ran, succeeded, and passed its health check, so a lint-only run
can never become a rollback target. Three guards prevent loops, including a
per-environment throttle; per-*target* throttling cannot work, because each successful
rollback becomes the new last-known-good and the target rotates every cycle.

When a policy is configured but a guard refuses it, the run records
`rollback_outcome: skipped` plus the reason — that is the case where the broken release is
still live, so it must not look like "no policy configured".

### 3. Local triggers

`.chibby/triggers.toml`, layered with a gitignored `.chibby/triggers.local.toml` so a
schedule can be per-machine rather than fired on every teammate's checkout.

- **Schedules** — cron (5- and 6-field forms), with a missed-run policy. A newly added
  schedule fires forward rather than immediately, and `run_once` fires **exactly once** no
  matter how many occurrences were missed: a laptop closed for a week must not wake up and
  run seven deploys.
- **Watches** — glob include/exclude with debounce. `.git/` and `.chibby/` are always
  excluded; the latter is mandatory, since runs write there and omitting it is an infinite
  loop.
- **Git hooks** — sentinel-delimited blocks that refuse to clobber an existing hook,
  offering append or backup-and-replace instead, and failing open if the binary is missing
  rather than bricking `git push`.

Runs record `run_kind` (`scheduled` / `watch` / `hook`) and `trigger_id`. Unattended
failures escalate their notification regardless of config, because nobody is watching.

A cross-process lock in the data directory means the desktop app and a headless
`chibby schedule` cannot double-fire the same trigger. That lock also made `chibby cancel`
real — it previously just printed "press Ctrl-C", because there was no daemon to signal.

**These fire only while something is running** — the desktop app, or `chibby schedule` /
`chibby watch` in a terminal. There is no background daemon. `chibby schedule --once` is
the hook for launchd / systemd / Task Scheduler.

## Still open

The Tier-0 rows not marked FIXED, all of Tier 1 #4 (insight), and all of Tier 2. The
insight work is the natural next step: it needs the git provenance that now exists, and it
can reuse `DashboardOverview.tsx`, but it also wants a charting decision and a fix for the
`load_runs()` scaling problem first.
