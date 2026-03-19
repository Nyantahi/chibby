# Chibby Phased Build Roadmap

Status: In Progress
Date: 2026-03-16
Last Updated: 2026-03-16

## Purpose

This roadmap turns the Chibby concept into a build sequence that preserves the
product wedge: local-first, script-first, native-friendly CI/CD for solo
developers and tiny teams.

The roadmap is phased instead of date-driven so it can work for a single
developer or a small open-source effort. Each phase has a clear objective,
deliverables, and exit criteria.

## Guiding Rules

- Preserve local-first value in every phase.
- Do not build shared infrastructure before the single-user workflow is strong.
- Prioritize import of existing scripts over creation of a complex DSL.
- Keep the Rust engine reusable outside the Tauri UI.
- Delay team and platform features until core deploy workflows are solid.

## Phase 0: Dogfood on Okapian [DONE]

Completed: 2026-03-16

Objective:

Validate the wedge by solving real deployment problems in the Okapian project
itself, rather than running abstract user research first.

Okapian is the ideal proving ground because it has exactly the pain points
Chibby targets:

- a Tauri desktop app with zero release automation (no signing, no
  notarization, no update distribution, no artifact pipeline)
- a website with CI workflows that only test, never deploy
- manual terminal-driven build and deploy loops
- secrets in `.env` files instead of a keychain
- no rollback capability and no deployment history
- version numbers out of sync across config files

Deliverables:

- audit of Okapian's current deploy workflow for both the desktop app and the
  website
- list of concrete deploy steps Chibby must automate to replace the manual
  workflow
- rough UI wireframes for onboarding, run view, and rollback
- one narrow first target: Okapian Tauri app build and release from a single
  machine
- one secondary target: Okapian website Docker build and SSH deploy

Exit criteria:

- both target workflows are documented as step-by-step command sequences
- the pipeline format can represent both workflows
- clear confirmation that importing existing scripts and wrapping them in a UI
  is more compelling than writing pipeline config from scratch

Audit results: See [phase-0-audit.md](../phase-0-audit.md)

## Phase 1: Core Engine Prototype [DONE]

Completed: 2026-03-16

Objective:

Prove that the runtime model works before investing heavily in the UI. Build
cross-platform from the start so platform-specific issues surface early.

Deliverables:

- Rust core library for stage execution
- basic CLI wrapper around the core engine
- local process execution backend
- stage status and log streaming
- local persistence for run metadata
- cross-platform shell abstraction (sh/bash on macOS/Linux, cmd/PowerShell on
  Windows)
- cross-platform data directory resolution (XDG on Linux, Application Support
  on macOS, AppData on Windows)

Exit criteria:

- commands can be executed as ordered stages
- output streams in real time
- run status, timestamps, durations, and exit codes are stored reliably
- the engine compiles and passes tests on macOS, Linux, and Windows

Notes:

This phase should happen before advanced UI work. If the runtime feels wrong,
the product will feel wrong regardless of interface quality.

Cross-platform support must be a day-one concern, not a retrofit. Use trait
abstractions for shell execution, credential storage, and file paths from the
first commit.

## Phase 2: Script Detection and Pipeline Generation [DONE]

Completed: 2026-03-16

Objective:

Make onboarding materially easier than writing CI config by hand. Include
LLM-assisted pipeline generation as part of the onboarding flow, not as a
later add-on.

Deliverables:

- repo import flow
- script detection for `deploy.sh`, `Makefile`, `justfile`, `Dockerfile`,
  `docker-compose.yml`, and package scripts
- heuristic stage generation for build, test, and deploy
- LLM-assisted first-pass pipeline generation from repo contents (optional,
  falls back to heuristics when unavailable)
- minimal pipeline format (TOML) and editor

Exit criteria:

- a user can point Chibby at a repo and get a first runnable pipeline with no
  hand-written config in common cases
- generated pipelines are editable and understandable
- LLM-assisted generation produces a better draft than heuristics alone for
  repos with complex script structures

Risks:

- script detection can become too magical and unreliable
- pipeline generation can drift into brittle guesswork
- LLM-assisted generation may produce incorrect or unsafe commands

Mitigation:

- keep the generated model simple
- always show the inferred pipeline before execution
- LLM-generated pipelines must be clearly marked as drafts requiring user
  review
- the feature must work without LLM access; heuristics are the baseline

## Phase 3: Tauri Desktop Shell and Basic UX [DONE]

Completed: 2026-03-16

Objective:

Wrap the runtime in a usable product experience.

Deliverables:

- Tauri application shell
- repo list and repo details view
- pipeline graph or stage list
- run view with live logs
- basic settings and local storage management

Exit criteria:

- a user can import a repo and run a pipeline entirely through the desktop UI
- the UI is fast, readable, and does not hide core execution details

Notes:

The UX should feel like a deploy console, not a generic admin dashboard.

## Phase 4: Environments, Secrets, and SSH Deploys [DONE]

Completed: 2026-03-16

Objective:

Support the most common real-world deploy paths for the target user, including
both direct SSH command execution and Docker-based deployments over SSH.

Deliverables:

- environment definitions such as `dev`, `staging`, and `production`
- per-environment variables and secret references
- OS keychain integration where possible
- SSH backend support for direct command execution
- SSH backend support for remote `docker compose` orchestration
- health check validation after deployment completes
- service-level rollback awareness for multi-service Docker deployments
- preflight validation for missing configuration

Exit criteria:

- user can define at least one remote environment and deploy over SSH
- user can run `docker compose up` on a remote host as a deploy step
- secrets are not stored in plaintext in repo configuration
- configuration errors are surfaced before a run starts
- a post-deploy health check can verify that services are responding

Notes:

This phase is where Chibby begins to replace ad hoc terminal deploy habits.
Docker-over-SSH is the most common deploy pattern for solo developers hosting
on a VPS and must be first-class alongside direct SSH command execution.

## Phase 5: Versioning, Signing, and Artifact Management [DONE]

Completed: 2026-03-17

Objective:

Complete the deployment lifecycle with the steps that sit between "it built" and
"it deployed." Without these, builds are unnamed blobs, users get OS security
warnings, and there is no reliable way to know what was shipped.

Deliverables:

### Versioning and tagging

- version bump command that updates all relevant config files in sync
  (`package.json`, `Cargo.toml`, `tauri.conf.json`, and any user-specified files)
- support for semver bump levels: patch, minor, major, and explicit version
- automatic git tag creation after a successful version bump
- optional changelog generation from commit messages since last tag
- version validation that prevents deploying an already-released version

### Code signing and notarization

- macOS: code signing with Developer ID and Apple notarization via `notarytool`
- Windows: Authenticode signing support via `signtool` or equivalent
- Linux: optional GPG signing for packages
- signing credentials stored via the existing secrets/keychain system (Phase 4)
- clear error messages when signing identity is missing or expired
- skip-signing flag for local development builds (clearly marked as unsigned)

### Artifact management

- artifact output directory per pipeline run, named consistently
  (`{project}-{version}-{platform}-{arch}.{ext}`)
- SHA256 checksum generation for every artifact
- local artifact storage with configurable retention (keep last N versions)
- artifact manifest that records what was built, when, and from which commit
- optional upload step to external storage (S3, GitHub Releases, SCP to server)

### Notification

- post-run notification hooks: success, failure, or both
- built-in support for desktop OS notifications
- webhook support for Slack, Discord, and generic HTTP POST
- notification content includes: project name, version, environment, status,
  duration, and link to logs
- notifications are optional and off by default

### Cleanup

- automatic pruning of old artifacts beyond retention limit
- remote cleanup commands for Docker image pruning on deploy targets
- log rotation for local run history (configurable max age or count)
- dry-run mode for cleanup so users can preview what would be removed

Exit criteria:

- a version bump propagates to all configured files and creates a git tag
- a macOS build can be signed and notarized in a single pipeline run
- artifacts are stored with consistent names and verifiable checksums
- a user receives a notification after a deploy without manual setup beyond
  initial configuration
- old artifacts and logs do not accumulate indefinitely

Risks:

- code signing setup varies wildly across developer machines and credentials
- artifact upload to external services introduces network failure modes
- notification integrations can be flaky and distract from core value

Mitigation:

- signing is opt-in per pipeline; unsigned builds still work
- artifact upload is a separate optional stage; local storage is the default
- notification failures are logged but never block a pipeline run
- provide clear setup guides for each platform's signing requirements

Notes:

These five capabilities turn Chibby from "it runs commands" into "it ships
software." Versioning and artifacts are prerequisites for meaningful rollback
in Phase 6, since rollback needs to know what version to revert to and where
the previous artifact is.

## Phase 5.5: Tauri Updater Integration [DONE]

Completed: 2026-03-17

Objective:

Close the gap between "it built and signed the app" and "users receive the
update." Tauri's built-in updater plugin requires a signed update bundle and a
`latest.json` endpoint. Today this is one of the most painful manual steps for
Tauri developers — generating the right JSON, managing the update signing key
pair, and uploading everything to a hosting provider. Chibby should make this a
single pipeline stage.

This phase sits between Phase 5 (which produces signed artifacts) and Phase 6
(which tracks run history), because update distribution is a natural extension
of artifact management and must work before rollback is meaningful for desktop
apps.

Deliverables:

### Tauri update key management

- generate a Tauri update key pair (`tauri signer generate`) and store the
  private key in the OS keychain via the existing secrets system (Phase 4)
- store the public key reference in `.chibby/updater.toml` alongside the
  project config
- preflight check that warns if the update private key is missing before a
  release pipeline starts
- key rotation support: generate a new key pair, re-sign the current release,
  and update `latest.json` in a single operation

### `latest.json` generation

- after a successful build + code signing stage, automatically generate a
  Tauri-compatible `latest.json` from the artifact manifest
- populate all required fields: `version`, `notes` (from changelog), `pub_date`,
  and per-platform `url` + `signature` entries
- support multi-platform `latest.json` (macOS `.app.tar.gz`, Windows `.msi.zip`,
  Linux `.AppImage.tar.gz`) with correct platform keys
- the generated file is placed in the artifact output directory alongside the
  build artifacts
- validate the generated JSON against the Tauri updater schema before upload

### Update bundle signing

- sign the update bundle (`.tar.gz` of the app) with the Tauri update private
  key — this is separate from macOS code signing / Windows Authenticode
- include the base64 signature in `latest.json` per Tauri's expected format
- verify the signature locally before publishing as a sanity check

### Update publishing

- upload `latest.json` + update bundles to a configurable hosting target:
  - Cloudflare R2
  - AWS S3 (or S3-compatible)
  - GitHub Releases (attach assets + update `latest.json` as a release asset)
  - SCP to a static file server
  - local directory (for self-hosted or LAN distribution)
- each hosting target is a simple config block in `.chibby/updater.toml`
- upload is atomic where possible: write to a temp path, then rename, so
  clients never fetch a partial `latest.json`
- support a `--dry-run` flag that generates `latest.json` and shows what
  would be uploaded without actually publishing

Exit criteria:

- a Tauri app project can go from `bump → build → sign → publish update` in a
  single pipeline run
- the generated `latest.json` is valid and the Tauri updater plugin can consume
  it without manual edits
- update signing keys are stored securely and never written to disk in plaintext
- at least one hosting target (S3-compatible or GitHub Releases) works
  end-to-end
- a developer who has never configured Tauri updates before can complete setup
  through the Chibby UI with clear guidance at each step

Risks:

- Tauri updater format may change between Tauri v2 minor releases
- hosting provider auth adds yet more secrets to manage
- multi-platform `latest.json` requires building on multiple machines or
  cross-compilation, which Chibby does not orchestrate yet

Mitigation:

- pin to the Tauri v2 updater JSON schema and version-check at generation time
- hosting credentials go through the existing secrets/keychain system — no new
  secret storage mechanism needed
- for multi-platform, support merging per-platform `latest.json` fragments into
  a combined file, so each machine can build its platform and merge later
- the feature must work for single-platform releases (the common solo-dev case)
  without requiring cross-platform builds

Notes:

This is the feature that directly solves the Reddit pain point. Every Tauri
developer hits the same wall: the app builds, but getting updates to users
requires stitching together signing keys, JSON files, and upload scripts by
hand. Chibby already has all the prerequisites — version bumping, code signing,
artifact collection, and upload destinations. This phase connects them into a
complete update distribution pipeline.

## Phase 5.8: Security and Quality Gates [DONE]

Completed: 2026-03-17

Objective:

Add automated security scanning and commit hygiene as built-in pipeline stages,
so projects get the safety nets that mature CI/CD systems provide without
requiring the developer to wire up separate tools and GitHub Actions workflows.

These gates run as optional preflight or pre-deploy checks within a Chibby
pipeline. They are not enforced by default but are easy to enable and hard to
misconfigure.

Deliverables:

### Secret scanning (Gitleaks)

- built-in pipeline stage that scans the repo for accidentally committed
  secrets using gitleaks (or an equivalent embedded scanner)
- scan triggers: before every deploy stage, and optionally on every run
- default ruleset covers API keys, tokens, passwords, private keys, database
  URLs with embedded credentials, absolute paths with usernames, and
  AWS/GCP/Azure credentials
- custom rules via `.chibby/gitleaks.toml` that extend or override the defaults
- clear output: file path, line number, rule that matched, and a redacted
  preview of the match
- configurable behavior on match: block the pipeline (default), warn and
  continue, or ignore specific paths/patterns via an allowlist
- first-run baseline mode that marks existing findings as acknowledged so
  only new leaks block the pipeline

### Dependency and CVE scanning

- built-in pipeline stage that checks project dependencies for known
  vulnerabilities
- language-specific adapters:
  - Rust: `cargo audit`
  - Node.js: `npm audit` / `pnpm audit`
  - Python: `pip-audit` or `safety`
  - Go: `govulncheck`
- scan triggers: before build or on a configurable schedule
- severity threshold: block on critical/high by default, warn on medium/low
- output includes: package name, installed version, fixed version (if
  available), CVE identifier, and severity rating
- configurable allowlist for known false positives or accepted risks, stored
  in `.chibby/audit-allowlist.toml`
- when a fix is available, suggest the upgrade command in the output

### Commit message linting (Commitlint)

- optional pipeline stage that validates commit messages against Conventional
  Commits format: `<type>(<scope>): <description>`
- supported types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`,
  `build`, `ci`, `chore`
- validation scope: commits since last tag, last deploy, or a configurable
  range
- clear error output: which commit failed, what rule it broke, and the
  expected format with examples
- configurable rules via `.chibby/commitlint.toml`:
  - required types (default: all Conventional Commits types)
  - max subject line length (default: 72)
  - require scope (default: false)
  - custom type allowlist for project-specific types
- enforcement mode: block (prevent deploy if commits are malformed), warn
  (report but continue), or off
- default is warn mode — strict enforcement is opt-in so it does not surprise
  new users
- integration with the existing changelog generation (Phase 5): well-formed
  commit messages produce better changelogs automatically

Exit criteria:

- a project with a committed `.env` file is flagged before deploy
- a project with a known CVE in a dependency is flagged before build
- malformed commit messages are reported with actionable fix guidance
- all three gates can be enabled with a single config change and disabled
  individually
- false positives can be suppressed without disabling the entire gate
- gates never block a pipeline silently — every block includes a clear
  explanation and a path to resolution

Risks:

- secret scanning can produce false positives that erode trust in the gates
- dependency scanners require language-specific tooling to be installed
- commitlint can feel heavy-handed for solo developers

Mitigation:

- baseline mode and allowlists keep false positives manageable
- dependency scanning gracefully degrades: if `cargo audit` is not installed,
  skip with a warning and suggest installation — never fail silently or block
  on a missing scanner
- commitlint defaults to warn mode, not block — the developer opts in to
  strict enforcement when they are ready
- all gates are off by default; the recommendations engine (already built)
  suggests enabling them based on project maturity

Notes:

These gates complement the existing preflight system (Phase 4) which validates
configuration (missing secrets, SSH connectivity). Security and quality gates
validate the code and dependencies themselves. Together they form a complete
pre-deploy safety net.

The gitleaks integration is especially important for Chibby's target user —
solo developers who handle their own secrets and may not have a team review
process to catch accidental commits.

## Phase 6: Run History, Retry, and Rollback [DONE]

Completed: 2026-03-18

Objective:

Deliver the operational safety that makes the product more than a script runner.

Deliverables:

### Run history and inspection

- persistent run history with full stage results, logs, and metadata
- run kind tracking: normal runs, retries, and rollbacks are tagged and
  distinguishable in the UI
- environment-filtered history view in the project detail page
- last known good deployment prominently displayed in the history tab
- deployment history per environment via dedicated query endpoint

### Retry from failed stage

- retry command that re-executes a pipeline starting from the first failed
  stage (or any user-specified stage)
- stages before the retry point are skipped; the new run is tagged as a retry
  with a link to the parent run
- retry attempt number is tracked (retry #1, #2, etc.)
- retry buttons in both RunDetail (per-stage and full retry) and the run
  history view
- environment variables and secrets are re-resolved at retry time so fixes
  to configuration take effect immediately

### Rollback

- rollback command that re-executes the full pipeline using the same
  environment as a previously successful run
- rollback is explicit: the user chooses which successful run to roll back to
- the resulting run is tagged as a rollback with a link to the target run
- rollback button appears on successful runs that deployed to an environment
- rollback audit trail: every rollback is a normal run record with `run_kind:
  rollback` and `rollback_target_id` linking to the target

### Data model extensions

- `PipelineRun` extended with: `run_kind`, `parent_run_id`, `retry_number`,
  `rollback_target_id`, `retry_from_stage`
- `DeploymentRecord` summary type for environment deployment timelines
- backward-compatible: all new fields use `#[serde(default)]` so existing
  run history files deserialize without migration
- persistence helpers: `last_successful_run()`, `deployment_history()`,
  `retry_count_for_run()`

### Frontend

- RunDetail: retry buttons (from failed stage, retry all), rollback button,
  run kind badges, parent/target run links
- ProjectDetail history tab: environment filter, last known good deployment
  card, retry/rollback badges on run rows
- new badge styles: info (retry), warning (rollback)
- new button style: warning variant for rollback actions

Exit criteria:

- a failed run can be retried without rerunning successful earlier stages
- a previous successful deployment can be identified and rolled back explicitly
- users can inspect historical logs and outcomes with minimal friction

Risks:

- rollback means different things across projects

Mitigation:

- keep v1 rollback explicit and command-based
- avoid pretending rollback is automatic when the repo has no clear rollback path

## Phase 7: Packaging, OSS Readiness, and Early Adopters [DONE]

Completed: 2026-03-18

Objective:

Turn the prototype into an open-source project others can try and contribute to
on all three major desktop platforms.

Deliverables:

- installation instructions for macOS, Linux, and Windows
- platform-specific install packages:
  - macOS: `.dmg` (signed and notarized)
  - Linux: `.deb`, `.rpm`, `.AppImage`
  - Windows: `.msi` or NSIS installer (signed)
- example repos and example pipelines
- docs for local setup, secrets, and SSH use on each platform
- issue templates and contribution guidelines
- CI pipeline that builds and tests on all three platforms
- Homebrew formula, AUR package, and/or winget manifest (stretch goals)

### Build-time validation and CI compatibility

- modular `build_checks/` directory in `src-tauri/` with per-platform validation
  modules that other contributors can extend without modifying unrelated code
- macOS checks:
  - `create-dmg` presence when DMG bundling is enabled
  - bundle identifier ending with `.app` (conflicts with macOS bundle extension)
  - code signing identity availability
  - notarization credentials in keychain
- Windows checks:
  - VBSCRIPT/WMI availability for MSI builds
  - WebView2 SDK or runtime presence
  - `signtool` availability for code signing
  - NSIS installation path for NSIS bundles
- Linux checks:
  - `libwebkit2gtk-4.1-dev` package presence
  - `libappindicator3-dev` package presence
  - `patchelf` availability for AppImage builds
  - GLIBC version compatibility warning for old distros
- cross-platform checks:
  - version sync across `package.json`, `Cargo.toml`, and `tauri.conf.json`
  - bundle identifier reverse-domain format validation
  - required environment variables for CI builds
- all checks emit `cargo:warning=` messages that surface during build without
  blocking; developers can promote warnings to errors via config flag
- checks are designed for both local builds and CI environments; CI-specific
  checks (missing secrets, unsigned builds on release branches) can be added
  by contributors in the appropriate platform module

Exit criteria:

- an external developer on macOS, Linux, or Windows can install and use Chibby
  without source spelunking
- at least a few early adopters on each platform can complete the core workflow
- platform-specific credential storage and shell execution work without manual
  workarounds

## Phase 7.5: Dashboard and Operational Insights

Objective:

Reintroduce a dedicated Dashboard page once there is enough operational data
to justify a standalone view. In Phase 7 the Dashboard was merged into the
Projects page as a compact stats bar because a separate page added a click
without adding value for users with only a few projects.

The Dashboard should return when it can answer questions the Projects page
cannot: "what shipped where?", "is anything about to break?", and "how is my
deploy health trending?"

Deliverables:

- deployment timeline showing what was shipped to which environment and when
- environment status matrix: which version is live on staging vs production
  per project, with quick links to the run that deployed it
- failure trend chart: runs per day over a configurable window (7d / 30d),
  with success vs failure breakdown, to surface flaky pipelines
- artifact disk usage summary with a one-click cleanup nudge when usage
  exceeds a configurable threshold
- credential and signing certificate expiration warnings surfaced proactively
  (e.g., Apple Developer cert expiring in 30 days)
- quick-action cards for the most common next step: retry last failure,
  rollback to last known good, run a deploy that has been queued

Exit criteria:

- the Dashboard provides at least three insights that are not visible on the
  Projects page
- the page loads within 200ms for a user with 10 projects and 500 run records
- all data is derived from existing local storage; no new backend services

Notes:

This phase is optional and should only ship when real usage data confirms
that users want a centralized operational view. Until then, the stats bar
on the Projects page is sufficient.

## Phase 8: Agent-Assisted Failure Recovery and Optimization

Objective:

Add agent features that help users understand and recover from failures. Pipeline
generation assistance has already shipped in Phase 2; this phase focuses on
runtime intelligence.

Deliverables:

- failure summaries in plain English
- suggestions for missing tools, commands, or secrets
- repeated-failure or flaky-step detection and hints
- log analysis that links failures to recent code changes when possible
- suggested rollback steps based on failure context

Exit criteria:

- users can still see raw logs and raw commands
- agent output is optional, reviewable, and clearly separate from execution
- agent features measurably reduce confusion in failure recovery

Notes:

This phase should not ship before the core workflow is already useful without AI.
Pipeline generation assistance (Phase 2) is deliberately separated from runtime
agent features here.

## Phase 9: Optional Shared Mode and Remote Agents

Objective:

Expand carefully from solo use into tiny-team collaboration.

Deliverables:

- optional remote agent binary
- shared config or sync strategy
- basic multi-user run visibility
- guarded trust model for remote execution

Exit criteria:

- shared use cases are supported without turning Chibby into a heavy central
  platform
- local-first mode remains the default and simplest path

Notes:

This phase is optional. It should only happen after the solo-developer workflow
is clearly working and differentiated.

## Suggested Sequencing Priorities

If time is limited, prioritize in this order:

1. Phase 1
2. Phase 2
3. Phase 4
4. Phase 5
5. Phase 5.5
6. Phase 5.8
7. Phase 6
8. Phase 3
9. Phase 7
10. Phase 8
11. Phase 9

Reasoning:

- runtime and script import are the product core
- SSH, environments, versioning, signing, and artifacts complete the deploy path
- Tauri updater integration (5.5) connects signing and artifacts into a complete
  update distribution flow — the single most painful step for Tauri developers
- security and quality gates (5.8) catch leaked secrets and vulnerable
  dependencies before they reach production — better to add these before
  rollback exists so bad deploys are prevented rather than reverted
- retry and rollback depend on versioning and artifact tracking from Phase 5
- the UI matters, but it should be built on top of a stable execution model

If UI-first momentum is important for demos, swap Phase 3 earlier, but do not
let it outrun the engine.

## Suggested Milestone Shape

### Milestone A: "It runs my scripts"

Includes:

- core engine
- local execution
- repo import
- generated draft pipeline (heuristic and LLM-assisted)

### Milestone B: "It deploys my app"

Includes:

- environments
- secrets
- SSH deploys (direct commands and Docker-over-SSH)
- preflight validation
- post-deploy health checks

### Milestone C: "It ships my app properly"

Includes:

- version bumping and git tagging
- code signing and notarization
- artifact naming, checksums, and storage
- deploy notifications
- old artifact and log cleanup
- Tauri updater integration (`latest.json`, update signing, publish to hosting)
- secret scanning (gitleaks), dependency/CVE scanning, commit message linting

### Milestone D: "It is safer than my shell history"

Includes:

- run history
- retry
- rollback
- last known good deployment

### Milestone E: "Other people can adopt it"

Includes:

- packaging
- docs
- example repos
- OSS project hygiene

## What To Avoid While Building

- turning the pipeline format into a full programming language
- requiring a server too early
- overbuilding team features before the solo workflow works
- making agent features core to basic execution
- chasing parity with Jenkins, GitHub Actions, or Kubernetes platforms
- treating Windows or Linux as second-class platforms that get fixed later
- writing macOS-only code without platform abstractions

## Near-Term Build Recommendation

If building starts now, the most pragmatic sequence is:

1. Audit Okapian's deploy workflow and document the exact command sequences.
2. Build the Rust engine and CLI first.
3. Add repo import, script detection, and LLM-assisted pipeline generation.
4. Prove one end-to-end deploy flow over SSH (Okapian website).
5. Prove one native app release flow (Okapian Tauri build).
6. Add version bumping, code signing, and artifact management.
7. Add Tauri updater integration (`latest.json`, update signing, publish).
8. Add security and quality gates (secret scanning, CVE scanning, commitlint).
9. Add notifications and cleanup automation.
10. Add run history, retry, and rollback.
11. Wrap it in a Tauri UI good enough for daily use.
12. Package it and test with early users.

This is the shortest path to proving that Chibby is more useful than a shell
script without becoming another heavyweight CI platform. Using Okapian as the
first test case means every phase validates a real workflow.

## Document Relationships

This roadmap is aligned to:

- [Chibby concept doc](./chibby-local-first-ci-cd.md)
- [Chibby v1 PRD](./chibby-v1-prd.md)
