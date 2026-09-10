# Chibby Documentation

Index of everything in `docs/`. Start here.

Chibby is a local-first CI/CD and deployment desktop app (Rust + Tauri v2 backend,
React + TypeScript frontend) for solo developers and small teams. See the
[root README](../README.md) for the feature list and how it compares to hosted CI.

## Guides

Task-oriented, start-to-finish.

| Doc                                                                | What it covers                                                                                                                 |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| [guides/installation.md](guides/installation.md)                   | Downloading and installing the pre-built app on macOS, Linux, and Windows, plus SSH and secrets setup per platform             |
| [guides/user-guide.md](guides/user-guide.md)                       | Walkthrough of the app: adding a project, the dashboard, project tabs, building and running pipelines, and reading run history |
| [guides/build-troubleshooting.md](guides/build-troubleshooting.md) | Common failures when building Chibby for production, and how to fix them                                                       |

## Features

Reference docs for individual subsystems.

| Doc                                                      | What it covers                                                                                                                              |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| [features/agent.md](features/agent.md)                   | The in-app CI/CD agent — the Advise/Act toggle, the tool-use loop, approval and diff preview, git-branch isolation, and provider selection  |
| [features/cli-commands.md](features/cli-commands.md)     | The standalone `chibby` CLI: building it, and the full command reference. Shares the engine with the desktop app                            |
| [features/env-secrets.md](features/env-secrets.md)       | `environments.toml`, `secrets.toml`, per-developer `.local` overrides, OS keychain storage, bootstrap, importers, and the leak scanner      |
| [features/security-gates.md](features/security-gates.md) | The seven project-scoped gates, `gates.toml`, warn-vs-block modes, and the three surfaces they run from (CLI, Quality tab, pipeline stages) |
| [features/templates.md](features/templates.md)           | Full-pipeline and stage-snippet templates, 3-layer resolution (project / user / built-in), variable substitution, and import/export         |

## Design

| Doc                                  | What it covers                                                                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) | Color tokens, typography, spacing scale, and component conventions. Dark-first palette, teal accent. Follow it when adding or changing UI |

## Examples

Sample `pipeline.toml` files in [examples/](examples/), one per project shape:

| Example                                                               | Pipeline                                                                             |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| [node-webapp](examples/node-webapp/pipeline.toml)                     | Node.js web app — install, lint, test, build, SSH deploy                             |
| [python-django](examples/python-django/pipeline.toml)                 | Django app — lint, test, migrations check, SSH deploy                                |
| [rust-cli](examples/rust-cli/pipeline.toml)                           | Rust CLI — format check, clippy, test, release build                                 |
| [static-site](examples/static-site/pipeline.toml)                     | Static site (Hugo, Astro, Next.js export) — build locally, deploy via rsync over SSH |
| [tauri-desktop](examples/tauri-desktop/pipeline.toml)                 | Tauri desktop app — install, quality checks, test, build, optional signing           |
| [docker-compose-deploy](examples/docker-compose-deploy/pipeline.toml) | Build images locally, push to a registry, deploy with Docker Compose over SSH        |

## Planning

| Doc                                                            | What it covers                                                                                                                                                           |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [roadmap.md](roadmap.md)                                       | Ranked gap analysis from a codebase audit plus 2026 research — defect-grade holes, missing CI fundamentals, what's deliberately out of scope, and what's being built now |
| [proposals/env-secrets-epic.md](proposals/env-secrets-epic.md) | Working notes comparing the seven-category "full" 2026 security pipeline against what Chibby's gate engine covers, with a phased proposal for closing the gap            |

## Community

| Doc                                                          | What it covers                                                 |
| ------------------------------------------------------------ | -------------------------------------------------------------- |
| [community/CHANGELOG.md](community/CHANGELOG.md)             | Release history in Keep a Changelog format, semver             |
| [community/CONTRIBUTING.md](community/CONTRIBUTING.md)       | Development setup, prerequisites, workflow, and code style     |
| [community/CODE_OF_CONDUCT.md](community/CODE_OF_CONDUCT.md) | Contributor Covenant                                           |
| [community/SECURITY.md](community/SECURITY.md)               | Supported versions and how to report a vulnerability privately |

## Assets

- `logo/` — SVG marks and wordmark
- `screenshots/` — app screenshots used by the root README

## Conventions

Documentation filenames should be **kebab-case** (`user-authentication.md`). Some
existing files predate that rule and are SHOUTY_CASE — `DESIGN_SYSTEM.md`, and the four
files under `community/`. They are left as-is because they are linked from the root
README, from GitHub's community-health UI, and from external sites. New docs follow
kebab-case.
