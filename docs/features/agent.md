# CI/CD Agent

Chibby ships an in-app CI/CD agent: an AI assistant that investigates your project and
pipeline, explains what's wrong, and — when you let it — fixes it. It lives in a docked,
resizable **drawer** available from anywhere in the app, and as a **per-run analysis panel**
attached to a finished run.

The agent is **advisory-first**. By default it only reads: it inspects files, lists
directories, and validates your pipeline, then gives grounded advice and copy-pasteable next
steps. It changes nothing until you explicitly switch it into **Act** mode.

## Advise vs. Act

The drawer has a per-session toggle:

| Mode | What it does | Tools |
| --- | --- | --- |
| **Advise** (default) | Read-only. Investigates the project and pipeline and proposes exact commands / file changes without running anything. | `read_file`, `list_dir`, `validate_pipeline` |
| **Act** | Everything Advise can do, plus running commands and editing CI/CD files — each gated by your autonomy mode (below). | adds `run_command`, `edit_ci_file` |

The toggle defaults to **Advise** every session, so the agent never mutates your repo
unless you opt in.

## Autonomy (how much Act runs on its own)

Within Act mode, an **autonomy mode** (set in Settings) decides how much runs automatically
versus pausing for your approval:

| Autonomy mode | Behaviour |
| --- | --- |
| **Propose & approve** | The agent proposes every command and file edit; nothing runs until you approve. |
| **Auto-run safe, gate risky** | Safe commands and edits run automatically; deploys, pushes, and destructive actions pause for approval. |
| **Autonomous with checkpoints** | The agent runs multi-step sequences on its own, pausing only at checkpoints. |

## Safety guarantees

Act mode is designed so the auto-run path can never do something catastrophic:

- **Safe-allowlist floor** — the risky-command classifier only auto-runs commands it
  recognises as safe. Unknown commands and chained commands (`&&`, `;`, pipes) are gated for
  approval rather than assumed safe.
- **Catastrophic commands are always blocked** — regardless of autonomy mode (e.g. recursive
  deletes, `-delete`/`-exec`, package installs, credential reads are gated or refused).
- **Approval + diff preview** — file edits are shown as a diff and require approval before
  they're written (per your autonomy mode).
- **Git-branch isolation** — Act-mode changes are made on an isolated branch, so your working
  branch is never modified in place.
- **Secret redaction** — the agent's log sanitization redacts secret-shaped values so tokens
  and keys don't leak into transcripts.

## Providers, models & API keys

The agent talks to a hosted LLM. Configure this in **Settings → Agent**:

- **Provider** — `Auto` (Anthropic, falling back to OpenAI), `Anthropic`, or `OpenAI`. Use a
  specific provider to force one when you have keys for both. If the chosen provider's key is
  missing, the agent fails with a clear error rather than silently switching.
- **Model** — choose the model (e.g. Claude Opus for the most capable, Claude Sonnet for
  faster/cheaper).
- **API keys** — add an Anthropic and/or OpenAI key in Settings; keys are stored in your OS
  keychain, never in config files.

## Where to use it

- **Agent drawer** — a docked, resizable panel you can open on any project. Ask it to explain
  a failing stage, propose a fix, or set up a missing CI file. It streams its work (tool calls
  and output) as it goes.
- **Per-run analysis panel** — open a finished run and let the agent analyze it: it reads the
  run's stages and logs and surfaces findings plus suggested actions.

## Headless pipeline generation (`--ai`)

The same agent powers AI pipeline generation from the CLI — no GUI required:

```bash
# Generate a pipeline with AI assistance and write .chibby/pipeline.toml
chibby pipeline generate --ai

# Initialize a project with an AI-generated pipeline
chibby init --ai
```

Both summarize the project, call the agent to generate a pipeline, and write
`.chibby/pipeline.toml`. They honour the same provider/key configuration as the desktop app.

## See also

- [CLI reference — pipeline generation](cli-commands.md) — the `--ai` flags
- [Security & Quality Gates](security-gates.md) — the agent can help wire these into your pipeline
- [Environments & Secrets](env-secrets.md) — where deploy-time config and keychain-backed secrets live
