A "full" 2026 security pipeline pulls from these 7 categories. Times are typical CI cost:
  
  #: 1
  Category: Secret scanning      
  Common tools: gitleaks, trufflehog, GitHub Secret Scanning
  Time: ~30s
  What it catches: Hardcoded API keys, tokens, private keys in code/git history
  ────────────────────────────────────────
  #: 2
  Category: Dependency audit (SCA) 
  Common tools: npm audit, cargo audit, pip-audit, osv-scanner, Dependabot, Renovate, Snyk
  Time: 1-2min
  What it catches: Known CVEs in your direct + transitive deps
  ────────────────────────────────────────
  #: 3
  Category: SAST (Static Analysis)
  Common tools: CodeQL, Semgrep, SonarQube, Snyk Code, Bandit (Python), ESLint security plugins
  Time: 3-10min
  What it catches: Insecure code patterns: SQLi, XSS, command injection, hardcoded crypto
  ────────────────────────────────────────
  #: 4
  Category: Container scanning
  Common tools: Trivy, Grype, Snyk Container
  Time: ~2min
  What it catches: Vulnerabilities in OS packages + base images in Dockerfiles
  ────────────────────────────────────────
  #: 5
  Category: IaC scanning
  Common tools: Checkov, tfsec, KICS, Trivy IaC
  Time: ~1min
  What it catches: Misconfigured Terraform/K8s/Dockerfile/CloudFormation (open S3, no encryption, etc.)
  ────────────────────────────────────────
  #: 6
  Category: License compliance
  Common tools: license-checker (npm), cargo-license, FOSSA
  Time: <1min
  What it catches: GPL viral licenses in deps, license incompatibility
  ────────────────────────────────────────
  #: 7
  Category: DAST (Dynamic)
  Common tools: OWASP ZAP, Nuclei
  Time: 10-30min
  What it catches: Runtime vulns against a deployed staging instance
  
  Plus operational hygiene that's part of CI but not a "scanner":
  - Pinning action SHAs (actions/checkout@<sha> not @v4) — Trivy CI was hijacked this way in March 2026
  - OIDC instead of static cloud creds (id-token: write job permission) 
  - Minimum-permission GITHUB_TOKEN at workflow + job scope 
  - Branch protection + CODEOWNERS (not a workflow scanner but governs who can ship)
  
  ---
  What bituntu already runs
  
  Looking at your .github/workflows/:
  
  ┌────────────────────┬────────────────────────┬───────────────────────────────────────────────────────────────────────┐
  │       Check        │         Where          │                                 Tool                                  │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Container scan     │ build-push.yml         │ aquasecurity/trivy-action ✓                                           │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Python SAST        │ backend-testing.yml    │ bandit ✓                                                              │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Python dep audit   │ backend-testing.yml    │ pip-audit, safety ✓                                                   │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Dependency updates │ .github/dependabot.yml │ Dependabot (npm + pip) ✓                                              │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Secret scanning    │ —                      │ missing                                                               │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Frontend dep audit │ —                      │ missing (npm audit not wired)                                         │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ Frontend SAST      │ —                      │ missing (no ESLint-security, no CodeQL, no Semgrep)                   │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ IaC scanning       │ —                      │ missing (you have Dockerfiles + docker-compose; no checkov/trivy IaC) │
  ├────────────────────┼────────────────────────┼───────────────────────────────────────────────────────────────────────┤
  │ License check      │ —                      │ missing                                                               │
  └────────────────────┴────────────────────────┴───────────────────────────────────────────────────────────────────────┘
  
  So bituntu covers ~50% of the typical stack — solid on Python, light on frontend, no secret-scanning, no IaC.
  
  ---
  What Chibby's gates.toml covers today
  
  I checked the engine — chibby_lib::engine::gates:
  
  ┌─────────────────────┬──────────────────────┬─────────────────────────────────────────────────────────────────────────────────────────┐
  │     Chibby gate     │       Maps to        │                                          Wraps                                          │
  ├─────────────────────┼──────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────┤
  │ secret_scanning     │ Category 1           │ gitleaks if installed, else a built-in regex scanner                                    │
  ├─────────────────────┼──────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────┤
  │ dependency_scanning │ Category 2           │ cargo audit + npm audit/pnpm audit + pip-audit (auto-picks based on what's in the repo) │
  ├─────────────────────┼──────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────┤
  │ commit_lint         │ (style not security) │ Conventional commits                                                                    │
  └─────────────────────┴──────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────┘
  
  So Chibby covers 1, 2, and partial commit style — same gap as bituntu's workflows: no SAST, no container scan, no IaC scan, no license check.
  
  The reason "I don't see any of my projects [with checks] created in Chibby" — adding a project doesn't auto-create .chibby/gates.toml. You have to opt in (chibby gates init or via the Quality tab in the
  desktop app). None of your tracked projects currently has one.
  
  ---
  Recommendation — what to add
  
  Phase 1 (quick wins, no Chibby changes needed):
  1. Create .chibby/gates.toml for bituntu — enable secret + dependency scans Chibby already supports. Install gitleaks (brew install gitleaks) and cargo-audit so the wraps work.
  2. Add npm audit to bituntu's frontend GitHub workflow (one job, ~5 lines). 
  3. Add gitleaks GitHub Action to bituntu — covers secret scanning that bandit/pip-audit miss.
  
  Phase 2 (Chibby engine gap — multi-week):
  4. Extend Chibby's gates.rs with:
  - SAST — wrap semgrep ci (lang-agnostic) or bandit/eslint-plugin-security
  - Container scan — wrap trivy image against any Dockerfile in the repo
  - IaC scan — trivy config . or checkov -d . 
  - License check — cargo-license / license-checker (npm)