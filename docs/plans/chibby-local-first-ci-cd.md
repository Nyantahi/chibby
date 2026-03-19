# Chibby: Local-First Open-Source CI/CD for Solo Developers

## Overview

Chibby is a proposed open-source CI/CD and deployment tool built around a simple
idea: most solo developers and tiny teams do not need a heavy server-and-runner
platform to ship software. They already have scripts, a repo, a laptop, and
often one VPS. What they lack is a clean control plane that turns existing
automation into a repeatable, visible, safer workflow.

The product should start as a local-first Tauri desktop app backed by a Rust
automation engine. It should import existing scripts, run them locally or over
SSH, manage secrets safely, and give users a clear deployment history with logs,
retry, and rollback.

This is not a replacement for enterprise CI platforms. It is a better fit for:

- solo developers
- tiny teams
- native app developers
- developers deploying with shell scripts
- teams that want control without SaaS cost or operational overhead

## Core Thesis

The opportunity is not "another open-source CI server."

The opportunity is:

"A local-first, script-first CI/deploy tool that helps solo developers turn
their existing scripts into visual, repeatable pipelines and handles native app
and SSH-based deployment workflows better than container-first systems."

Short version:

"The missing UI and runtime for your existing deploy scripts."

## Problem

Many developers avoid paid CI/CD services because:

- cost grows quickly
- setup feels heavier than the app they are deploying
- they already have working scripts
- native app build and release flows do not fit neatly into container-first CI
- they want more control over secrets, hosts, and release execution

The result is usually a messy but functional setup:

- `deploy.sh`
- `Makefile`
- `justfile`
- ad hoc Git hooks
- manual SSH commands
- one laptop plus one server

That works until the workflow becomes brittle, hard to inspect, or hard to hand
off. Chibby should solve that without forcing users into an enterprise model.

## Existing Idea, Refined

The earliest concept was a small Tauri app that wraps deployment scripts with a
UI. That is still the right starting point.

Instead of:

```bash
./deploy.sh production
```

The user gets:

- a repo view
- environment targets like `dev`, `staging`, and `production`
- one-click run / deploy / rollback
- live logs and step status
- a simple history of what ran and what succeeded

Under the hood, the same commands still run.

## Product Principles

### 1. Local-first

The product must provide value on one machine with no server required.

### 2. Script-first

Users should be able to import what they already have before writing any new
pipeline definition.

### 3. Native-first

Local execution and SSH execution should be first-class. Containers should be
optional, not mandatory.

### 4. Open-source and self-directed

The tool should not require a hosted service to be useful.

### 5. Agent-assisted, not AI-dependent

AI can help with onboarding, failure explanation, and suggestions, but the core
workflow must remain transparent and deterministic.

### 6. Small-team-first

The first user should be one developer. The second user should be a tiny team.
The architecture can expand later, but the product must not start with team
complexity.

## What Existing Tools Already Do

Open-source CI/CD is already crowded. The key tools are not bad; they are often
misaligned with the needs of a solo developer.

### Jenkins

Strengths:

- extremely flexible
- huge plugin ecosystem
- mature controller/agent model
- strong pipeline support with `Jenkinsfile`

Weakness:

- too much operational and conceptual overhead for a single developer using a
  few scripts

### GoCD

Strengths:

- strong modeling of stages, jobs, agents, and environments
- very good pipeline visualization
- value stream map is a strong concept

Weakness:

- still assumes a fuller CI platform mindset than many solo developers need

### Drone / Harness

Strengths:

- clean repo-centric pipeline model
- YAML-first workflows
- multiple runner options

Weakness:

- still shaped around server/runner concepts and not clearly centered on the
  "I already have scripts" workflow

### Woodpecker

Strengths:

- lighter-weight than many alternatives
- open-source
- simpler than Jenkins and some other older systems

Weakness:

- still CI platform shaped, still YAML and runner oriented, still more setup
  than a solo developer may want

### Concourse

Strengths:

- very explicit model
- strong reproducibility and pipeline discipline

Weakness:

- conceptually powerful but not beginner-friendly

### Gitea Actions / Forgejo Actions

Strengths:

- natural choice when users already self-host their forge
- familiar workflow model for GitHub Actions users

Weakness:

- best when CI is part of the forge, not when the core problem is local script
  automation and deploy orchestration

### Dagger

Strengths:

- closest to a modern local-first workflow engine
- same logic can run locally and in CI
- promising direction for agent-assisted development workflows

Weakness:

- more programmable and infrastructure-oriented than users who simply want
  "import my deploy script and give me a UI"

## Pain Points Across the Market

### 1. Setup overhead

Many tools require some combination of:

- a central server
- agents, workers, or runners
- webhooks
- OAuth app configuration
- secret distribution between systems
- database or persistent service setup

That is not acceptable friction for a single engineer trying to ship from one
repo to one server.

### 2. Users must learn the tool before they get value

Each system introduces its own concepts and workflow model. That is reasonable
for team platforms. It is poor UX for a script-based deploy user.

### 3. Container-first assumptions exclude important workflows

Container execution is useful, but not universal. It is often awkward for:

- Tauri desktop apps
- macOS signing and notarization
- Windows signing
- native desktop packaging
- SSH-based deployments
- projects that need host-level credentials or tooling

### 4. Security complexity shows up early

Trust boundaries, secret scope, privileged runners, host mounts, and remote
agent concerns all show up quickly in existing tools. A simpler product should
reduce the surface area of those decisions early on.

### 5. Logs are visible but not necessarily helpful

Many systems display raw logs well enough but do little to explain:

- why a step failed
- whether a step is flaky
- what changed since the last successful run
- what the smallest likely fix is

### 6. Existing tools are built around teams and infrastructure

A solo engineer often wants:

- validate a branch before merging
- run tests locally
- deploy over SSH
- manage a few environments
- keep secrets local
- retry from failure
- rollback to the last known good release

That workflow is still underserved.

## Where Chibby Can Win

### Zero-server first run

The first-run experience should be:

1. Install app.
2. Select repo.
3. Detect scripts and package tasks.
4. Infer a draft pipeline.
5. Run locally.

No server should be required to get value.

### Best-in-class script import

The main onboarding path should not be "write YAML."

It should be:

1. import `deploy.sh`, `Makefile`, `justfile`, or package scripts
2. infer stages and dependencies
3. map required environment variables and secrets
4. let the user edit the generated pipeline if needed

### Native execution first

Execution backends should begin with:

- `local`
- `ssh`
- optional `container`

This is a much better fit for real solo-developer workflows than requiring
Docker as the default abstraction.

### Better deployment UX

The UI should focus on shipping software, not on administering CI infrastructure.

Important screens:

- repo onboarding
- pipeline view
- environment management
- run history
- live logs
- artifacts and releases
- secret management
- rollback and retry actions

### Useful agent assistance

Good uses of agents:

- generate a first-pass pipeline from repo contents
- summarize failures in plain English
- suggest likely fixes
- explain missing tools or secrets
- detect flaky steps or repeated failure patterns
- suggest rollback steps

Bad uses of agents in v1:

- hidden autonomous deploy actions
- opaque pipeline generation with no user review
- replacing deterministic execution with AI-driven guessing

## Feature Ideas

### Pipeline Runner

Let users define or import pipelines made of steps such as:

```yaml
pipeline:
  build:
    - npm install
    - npm run build
  test:
    - npm test
  deploy:
    - ./deploy.sh production
```

The engine executes each step, captures status and output, and stores run
history.

### Git Triggering

Possible triggers:

- manual run
- branch or commit detection
- local Git hook
- scheduled run
- webhook later if needed

### Logs Viewer

The app should provide a live terminal-style view with:

- step status
- timestamps
- failure boundaries
- searchable output
- summarized error state

### Multi-Environment and Multi-Server Deploy

Users should be able to define deployment targets such as:

- development
- staging
- production

Each target can map to:

- SSH host
- environment variables
- secrets
- deploy command
- rollback command

### Secret Manager

Secrets should be stored with OS keychain integration where possible.

Examples:

- `API_KEY`
- `DATABASE_URL`
- `DEPLOY_TOKEN`
- signing credentials

### Notifications

Optional notifications:

- desktop notifications
- Slack
- email

This is useful, but not core to the first release.

### Release History

The product should track:

- when a deployment happened
- which commit was deployed
- who or what triggered it
- logs and artifacts
- whether rollback succeeded

### Retry and Rollback

This is one of the highest-value workflow features.

Users should be able to:

- retry from a failed stage
- re-run the last successful deployment
- rollback using an explicit rollback command or artifact reference

## Architecture Direction

The UI must not contain the automation logic.

Recommended architecture:

- Rust core library
- Rust CLI
- Tauri desktop UI
- optional background daemon later
- optional remote agent later

High-level shape:

```text
Tauri UI (macOS / Linux / Windows)
  |
  +-- Rust core engine
        |
        +-- local process runner (sh/bash or cmd/PowerShell)
        +-- SSH runner
        +-- optional container runner
        +-- run history store (platform-appropriate data dir)
        +-- credential storage (platform-native keychain)
```

This keeps the product flexible:

- GUI for day-to-day use
- CLI for scripting
- headless automation later
- remote execution later without redesigning the system

## MVP Recommendation

The MVP should prove the wedge, not the entire platform.

### Include in v1

- Rust execution engine (cross-platform: macOS, Linux, Windows)
- Tauri UI (all three desktop platforms)
- local runner with platform-native shell execution
- SSH runner
- repo import flow
- script detection
- minimal editable pipeline format (TOML)
- live logs
- run history
- environment management
- platform-native credential storage:
  - macOS: Keychain Services
  - Linux: libsecret / Secret Service API
  - Windows: Windows Credential Manager
- retry from failed stage
- rollback to previous successful deployment

### Delay until later

- remote shared server mode
- runner fleet management
- plugin marketplace
- Kubernetes integration
- enterprise RBAC
- full webhook and forge automation
- advanced artifact storage
- multi-tenant cloud features

## Good Initial Positioning

Potential positioning lines:

- open-source local-first CI/CD for solo developers
- visual deploy runner for bash scripts and native app releases
- the missing UI for your deploy scripts
- CI/CD for the one-engineer team

Best practical positioning:

"The missing UI and runtime for your existing deploy scripts."

That explains the product faster than calling it a full CI platform.

## Anti-Goals

Chibby should not start as:

- a Jenkins replacement for large organizations
- a Kubernetes-native deployment platform
- a full Git forge
- a general plugin marketplace
- a cloud-first SaaS
- a brand new complex DSL that users must learn before deploying

## Why Tauri Fits

Tauri is strategically useful because it supports:

- desktop distribution on macOS, Linux, and Windows from a single codebase
- low resource usage compared to Electron
- Rust-native execution engine integration
- access to local system capabilities
- local credentials and keychain workflows
- platform-native window chrome and system tray on all three OSes
- built-in bundlers for `.dmg`, `.deb`, `.AppImage`, `.msi`, and `.nsis`

Compared with Electron, the footprint is smaller and the runtime story is more
aligned with a compact developer tool.

Critically, Tauri's cross-platform support means Chibby can reach developers
regardless of whether they work on macOS, Linux, or Windows. Since many solo
developers use Linux servers for deployment, and many use Windows or macOS for
development, first-class support on all three platforms is not optional -- it is
a core product requirement.

## Suggested Validation Work

Before building too much, validate the wedge directly.

### User research

Talk to 5 to 10 developers who currently deploy via:

- shell scripts
- `make`
- `just`
- manual SSH workflows
- ad hoc GitHub Actions

Questions to validate:

- Is "import my existing script and give me a UI" compelling?
- Is rollback more valuable than generic pipeline management?
- Do users want local-only first, or shared remote execution sooner?
- Which native or non-container workflows are most painful today?

### Prototype flow

Build a narrow prototype:

1. select repo
2. detect scripts
3. infer stages
4. run locally
5. deploy over SSH
6. show logs
7. store run history
8. retry or rollback

### Best proving ground

A Tauri app release workflow is a strong validation scenario because it stresses:

- native build requirements
- signing and release complexity
- artifact handling
- host tooling access

If Chibby works well there, it is likely solving a real gap.

## Future Expansion

Once the core wedge works, the project can expand carefully into:

- optional remote agents
- shared team mode
- pull request validation flows
- artifact and release publishing
- richer scheduling
- reusable pipeline templates
- better forge integration
- agent-generated repair suggestions tied to logs and diffs

The expansion path should preserve the local-first design rather than replacing
it with a heavy server model.

## Competitive References

These tools are useful references for direction and differentiation:

- Jenkins: ecosystem breadth and plugin maturity
- GoCD: visualization and value stream thinking
- Woodpecker: simplicity relative to older CI tools
- Concourse: reproducibility discipline
- Dagger: local-first and agent-oriented workflow direction

The gap Chibby should target is this combination:

- simpler than Woodpecker
- more local-first than Jenkins and GoCD
- more native-app-friendly than container-first CI tools
- more approachable than Dagger for script-based deploy users

## Sources

- Jenkins Pipeline: https://www.jenkins.io/doc/book/pipeline/
- Jenkins nodes and agents: https://www.jenkins.io/doc/book/managing/nodes/
- Jenkins credentials: https://www.jenkins.io/doc/book/using/using-credentials/
- Jenkins repository: https://github.com/jenkinsci/jenkins
- GoCD concepts: https://docs.gocd.org/current/introduction/concepts_in_go.html
- GoCD pipelines as code:
  https://docs.gocd.org/current/advanced_usage/pipelines_as_code.html
- GoCD value stream map:
  https://docs.gocd.org/current/navigation/value_stream_map.html
- GoCD agent installation:
  https://docs.gocd.org/current/installation/installing_go_agent.html
- Drone pipeline overview: https://docs.drone.io/pipeline/overview/
- Drone GitHub provider setup: https://docs.drone.io/server/provider/github/
- Drone Docker runner overview: https://docs.drone.io/runner/docker/overview/
- Drone CLI quickstart: https://docs.drone.io/quickstart/cli/
- Harness open-source repository: https://github.com/harness/harness
- Woodpecker intro: https://woodpecker-ci.org/docs/3.10/intro
- Woodpecker workflow syntax: https://woodpecker-ci.org/docs/usage/workflow-syntax
- Woodpecker agent configuration:
  https://woodpecker-ci.org/docs/administration/configuration/agent
- Woodpecker server configuration:
  https://woodpecker-ci.org/docs/administration/configuration/server
- Woodpecker volumes: https://woodpecker-ci.org/docs/usage/volumes
- Concourse documentation overview: https://concourse-ci.org/docs/
- Concourse quick start:
  https://concourse-ci.org/docs/getting-started/quick-start/
- Concourse tasks: https://concourse-ci.org/docs/tasks/
- Concourse resources: https://concourse-ci.org/docs/resources/
- Gitea Actions overview: https://docs.gitea.com/usage/actions/overview
- Gitea act runner: https://docs.gitea.com/usage/actions/act-runner
- Forgejo Actions overview:
  https://forgejo.org/docs/latest/user/actions/overview/
- Forgejo Actions security:
  https://forgejo.org/docs/latest/user/actions/security/
- Dagger toolchains: https://docs.dagger.io/core-concepts/toolchains
- Dagger LLM support: https://docs.dagger.io/features/llm/
- Dagger agent quickstart:
  https://docs.dagger.io/getting-started/quickstarts/agent
