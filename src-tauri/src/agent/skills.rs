use super::context::AnalysisContext;
use super::SkillMode;

// ---------------------------------------------------------------------------
// Skill detection from user message keywords
// ---------------------------------------------------------------------------

struct SkillKeywords {
    mode: SkillMode,
    keywords: &'static [&'static str],
}

const SKILL_KEYWORDS: &[SkillKeywords] = &[
    SkillKeywords {
        mode: SkillMode::FailureAnalysis,
        keywords: &[
            "fail", "error", "broke", "broken", "crash", "why did", "root cause",
            "flaky", "intermittent", "timeout", "exit code", "what went wrong",
        ],
    },
    SkillKeywords {
        mode: SkillMode::PipelineOptimization,
        keywords: &[
            "optimize", "faster", "slow", "speed up", "parallel", "cache",
            "improve", "reduce time", "too long", "bottleneck",
        ],
    },
    SkillKeywords {
        mode: SkillMode::SecurityReview,
        keywords: &[
            "security", "cve", "vulnerability", "audit", "secret scan",
            "leak", "credential", "compliance", "dependency audit",
        ],
    },
    SkillKeywords {
        mode: SkillMode::DeployTroubleshoot,
        keywords: &[
            "deploy", "ssh", "docker", "rollback", "health check", "connection refused",
            "permission denied", "remote", "server", "container",
        ],
    },
    SkillKeywords {
        mode: SkillMode::ProjectSetup,
        keywords: &[
            "setup", "configure", "missing tool", "install", "getting started",
            "new project", "recommend", "what do i need",
        ],
    },
    SkillKeywords {
        mode: SkillMode::PipelineGenerate,
        keywords: &[
            "generate pipeline", "create pipeline", "new pipeline", "github actions",
            "circleci", "drone", "ci config", "workflow file", "pipeline for",
        ],
    },
    SkillKeywords {
        mode: SkillMode::Execute,
        keywords: &[
            "run pipeline", "execute", "start build", "run build", "trigger",
            "kick off", "launch pipeline",
        ],
    },
];

/// Detect skill mode from user message keywords.
pub fn detect_skill_mode(msg: &str, ctx: &AnalysisContext) -> SkillMode {
    let lower = msg.to_lowercase();

    // First: check explicit keyword matches (longer phrases first for specificity)
    let mut best_match: Option<(SkillMode, usize)> = None;

    for entry in SKILL_KEYWORDS {
        let match_count = entry
            .keywords
            .iter()
            .filter(|kw| lower.contains(*kw))
            .count();

        if match_count > 0 {
            if best_match.is_none() || match_count > best_match.unwrap().1 {
                best_match = Some((entry.mode, match_count));
            }
        }
    }

    if let Some((mode, _)) = best_match {
        return mode;
    }

    // Second: fall back to context-based detection
    detect_skill_from_context(ctx)
}

/// Detect skill mode from run context alone (no user message).
pub fn detect_skill_from_context(ctx: &AnalysisContext) -> SkillMode {
    if ctx.is_failed_run() {
        if ctx.failed_on_deploy() {
            return SkillMode::DeployTroubleshoot;
        }
        return SkillMode::FailureAnalysis;
    }

    SkillMode::General
}

/// Return skill-specific guidance text injected into the system prompt.
pub fn skill_guidance(skill: &SkillMode) -> &'static str {
    match skill {
        SkillMode::FailureAnalysis => {
            "You are analyzing a build or deploy failure. Focus on:\n\
             1. Identify the root cause from logs (not just the symptom).\n\
             2. Check if this is a transient issue (network, timing) or structural (missing dependency, config error).\n\
             3. Look at stderr first — that's where most errors surface.\n\
             4. Compare against recent commits to see if a code change caused the failure.\n\
             5. If you see patterns from memory, reference the previous occurrence.\n\
             6. End with a concrete fix: a command, a file edit, or a config change."
        }
        SkillMode::PipelineOptimization => {
            "You are reviewing a pipeline for optimization opportunities. Focus on:\n\
             1. Stage ordering — can any stages run in parallel?\n\
             2. Caching — are dependencies being re-downloaded on every run?\n\
             3. Redundant steps — any duplicate or unnecessary stages?\n\
             4. Build times — which stage is the bottleneck?\n\
             5. Suggest specific changes with expected time savings."
        }
        SkillMode::SecurityReview => {
            "You are reviewing security gate results. Focus on:\n\
             1. Explain each finding in plain English — what's the risk?\n\
             2. For CVEs: is there a fixed version? What's the upgrade command?\n\
             3. For secret leaks: where was it found and how to remediate.\n\
             4. Prioritize by severity — critical first.\n\
             5. Never reveal actual secret values in your analysis."
        }
        SkillMode::DeployTroubleshoot => {
            "You are debugging a deployment failure. Focus on:\n\
             1. SSH connectivity — check host, port, key path, permissions.\n\
             2. Docker issues — image pull failures, compose syntax, port conflicts.\n\
             3. Health check failures — is the service actually starting?\n\
             4. Environment variables — are all required vars set?\n\
             5. Suggest a rollback strategy if the deploy is unrecoverable.\n\
             6. Check if the issue is transient (network) or structural (misconfiguration)."
        }
        SkillMode::ProjectSetup => {
            "You are helping set up a project for CI/CD. Focus on:\n\
             1. What project type was detected and what tools are needed.\n\
             2. Missing dependencies or tools that should be installed.\n\
             3. Recommended pipeline stages for this project type.\n\
             4. Security best practices (secret management, dependency scanning).\n\
             5. Be specific with commands for the user's platform."
        }
        SkillMode::PipelineGenerate => {
            "You are generating a pipeline configuration file. Focus on:\n\
             1. Detect the project type from the project info provided.\n\
             2. Generate a complete, working config in the requested format.\n\
             3. Include standard stages: install, lint, test, build.\n\
             4. Add deploy stage only if the project has deploy configuration.\n\
             5. Use best practices for the chosen CI platform.\n\
             6. Explain what each stage does after the config."
        }
        SkillMode::Execute => {
            "You are being asked to execute a pipeline. Focus on:\n\
             1. Confirm which pipeline and environment the user wants to run.\n\
             2. Non-deploy stages will execute automatically.\n\
             3. Deploy stages MUST pause and request user approval.\n\
             4. Report progress as stages complete.\n\
             5. If a stage fails, analyze the failure immediately."
        }
        SkillMode::General => {
            "You are answering a general CI/CD question. Be direct and practical.\n\
             If the question relates to a specific skill area, suggest that the user\n\
             ask more specifically so you can provide deeper analysis."
        }
    }
}
