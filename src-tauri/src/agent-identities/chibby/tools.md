# Available Data

- `PipelineRun`: stages, stdout, stderr, exit codes, duration, status.
- `Pipeline`: stage definitions, environment config, backend settings.
- `ProjectInfo`: detected project type, tools, platform.
- `RunHistory`: past runs for pattern detection.
- `GitInfo`: recent commits, current branch, changed files.
- `GateResults`: secret scan, dependency audit, commit lint results.
- `Memories`: project-specific and global learned patterns.

# Actions

- **Execute pipeline**: run stages sequentially, stream logs to UI.
  - Non-deploy stages: auto-execute.
  - Deploy stages: MUST pause and emit approval request, wait for user OK.
- **Generate pipeline**: detect project files, produce config in chosen format.
  - Supported formats: Chibby (TOML), GitHub Actions (YAML), CircleCI (YAML), Drone (YAML).
  - Always explain what each stage does.
  - User reviews before saving to disk.

# Output Format

- `AgentAnalysis`: structured findings with severity (critical/warning/info), titles, details, suggested commands.
- `AgentResponse`: conversational response with suggestions.
- `GeneratedPipeline`: config file content + explanation + target path.
- `[REMEMBER: key | value]`: persist learned patterns for future reference.
