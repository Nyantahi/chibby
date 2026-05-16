/**
 * File-type icons with brand colors, inspired by VS Code icon themes.
 * Each icon is a small inline SVG — no external dependencies.
 */

const S = 16; // icon size

function Icon({ color, children }: { color: string; children: React.ReactNode }) {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
      <g fill={color}>{children}</g>
    </svg>
  );
}

/** Colored text label icon — fallback for less common types. */
function LabelIcon({ color, label }: { color: string; label: string }) {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <rect x="0.5" y="2" width="15" height="12" rx="2" fill={color} opacity="0.15" />
      <text
        x="8"
        y="11"
        textAnchor="middle"
        fontSize="7"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill={color}
      >
        {label}
      </text>
    </svg>
  );
}

// ─── npm / Node ──────────────────────────────────────────────────
function NpmIcon() {
  return (
    <Icon color="#CB3837">
      <rect x="1" y="3" width="14" height="10" rx="1" />
      <rect x="3" y="5" width="3" height="6" fill="var(--color-surface, #161822)" />
      <rect x="4" y="5" width="1" height="4" fill="#CB3837" />
      <rect x="7" y="5" width="3" height="6" fill="var(--color-surface, #161822)" />
      <rect x="8" y="5" width="1" height="4" fill="#CB3837" />
      <rect x="11" y="5" width="2" height="6" fill="var(--color-surface, #161822)" />
    </Icon>
  );
}

// ─── Rust / Cargo ────────────────────────────────────────────────
function RustIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <circle cx="8" cy="8" r="6" fill="none" stroke="#DEA584" strokeWidth="1.5" />
      <circle cx="8" cy="8" r="2.5" fill="none" stroke="#DEA584" strokeWidth="1.5" />
      <line x1="8" y1="2" x2="8" y2="4.5" stroke="#DEA584" strokeWidth="1.2" />
      <line x1="8" y1="11.5" x2="8" y2="14" stroke="#DEA584" strokeWidth="1.2" />
      <line x1="2" y1="8" x2="5.5" y2="8" stroke="#DEA584" strokeWidth="1.2" />
      <line x1="10.5" y1="8" x2="14" y2="8" stroke="#DEA584" strokeWidth="1.2" />
    </svg>
  );
}

// ─── Go ──────────────────────────────────────────────────────────
function GoIcon() {
  return <LabelIcon color="#00ADD8" label="Go" />;
}

// ─── Python ──────────────────────────────────────────────────────
function PythonIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <path
        d="M8 1C5.5 1 5 2 5 3v1.5h3v1H3.5C2 5.5 1 7 1 9s1 3.5 2.5 3.5H5V11c0-1 .8-2 2-2h3c1 0 2-.8 2-2V3.5C12 2 10.5 1 8 1z"
        fill="#3572A5"
      />
      <path
        d="M8 15c2.5 0 3-1 3-2v-1.5H8v-1h4.5C14 10.5 15 9 15 7s-1-3.5-2.5-3.5H11V5c0 1-.8 2-2 2H6c-1 0-2 .8-2 2v3.5C4 14 5.5 15 8 15z"
        fill="#FFD43B"
      />
      <circle cx="6.5" cy="3.5" r="0.8" fill="#fff" />
      <circle cx="9.5" cy="12.5" r="0.8" fill="#fff" />
    </svg>
  );
}

// ─── Docker ──────────────────────────────────────────────────────
function DockerIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <path
        d="M1 8h2v2H1V8zm2.5 0H6v2H3.5V8zM6.5 8H9v2H6.5V8zM3.5 5.5H6v2H3.5v-2zM6.5 5.5H9v2H6.5v-2zM6.5 3H9v2H6.5V3zM9.5 5.5H12v2H9.5v-2z"
        fill="#2496ED"
      />
      <path
        d="M14.5 7.5c-.5-.3-1.5-.4-2.2-.2-.2-.8-.7-1.5-1.3-1.8l-.3-.2-.2.3c-.3.5-.4 1.2-.3 1.8.1.4.2.8.5 1.1-.4.2-.8.4-1.2.5-.6.1-1.2.2-1.8.2H.4l-.1.5c-.1.8 0 1.6.3 2.4.4.8 1 1.4 1.8 1.8 1 .4 2.5.6 3.8.4 1-.2 1.8-.5 2.6-1 .6-.4 1.2-.9 1.6-1.6.7-1 1.1-2.2 1.3-3.4.5 0 1 .1 1.5-.1.4-.2.6-.5.8-.8l.2-.3-.3-.2c-.5-.3-1-.4-1.4-.4z"
        fill="#2496ED"
      />
    </svg>
  );
}

// ─── Makefile ────────────────────────────────────────────────────
function MakeIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="3" width="14" height="10" rx="2" fill="#6D8C3C" opacity="0.15" />
      <text
        x="8"
        y="11"
        textAnchor="middle"
        fontSize="6"
        fontWeight="800"
        fontFamily="monospace"
        fill="#6D8C3C"
      >
        {'$>_'}
      </text>
    </svg>
  );
}

// ─── Shell Script ────────────────────────────────────────────────
function ShellIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="2" width="14" height="12" rx="2" fill="#4EAA25" opacity="0.15" />
      <path
        d="M4 6l3 2-3 2"
        stroke="#4EAA25"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <line
        x1="8"
        y1="11"
        x2="12"
        y2="11"
        stroke="#4EAA25"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

// ─── Env file ────────────────────────────────────────────────────
function EnvIcon() {
  return <LabelIcon color="#ECD53F" label=".env" />;
}

// ─── Maven ───────────────────────────────────────────────────────
function MavenIcon() {
  return <LabelIcon color="#C71A36" label="mvn" />;
}

// ─── Gradle ──────────────────────────────────────────────────────
function GradleIcon() {
  return <LabelIcon color="#02303A" label="G" />;
}

// ─── .NET ────────────────────────────────────────────────────────
function DotNetIcon() {
  return <LabelIcon color="#512BD4" label=".N" />;
}

// ─── PHP / Composer ──────────────────────────────────────────────
function PhpIcon() {
  return <LabelIcon color="#777BB4" label="PHP" />;
}

// ─── CMake ───────────────────────────────────────────────────────
function CMakeIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <polygon points="8,1 14,13 2,13" fill="none" stroke="#064F8C" strokeWidth="1.5" />
      <polygon points="8,5 11,11 5,11" fill="#064F8C" opacity="0.3" />
    </svg>
  );
}

// ─── Ruby / Gemfile ──────────────────────────────────────────────
function RubyIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <polygon points="8,1 14,6 12,15 4,15 2,6" fill="#CC342D" opacity="0.85" />
      <polygon points="8,3 12,6.5 10.5,13 5.5,13 4,6.5" fill="#CC342D" opacity="0.4" />
    </svg>
  );
}

// ─── GitHub Actions ──────────────────────────────────────────────
function GitHubIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <circle cx="8" cy="8" r="6.5" fill="#24292F" />
      <path
        d="M8 2.5a5.5 5.5 0 00-1.74 10.72c.28.05.38-.12.38-.26v-.94c-1.54.34-1.87-.74-1.87-.74a1.47 1.47 0 00-.62-.81c-.5-.35.04-.34.04-.34a1.17 1.17 0 01.85.57 1.18 1.18 0 001.62.46 1.18 1.18 0 01.35-.74c-1.23-.14-2.52-.62-2.52-2.74a2.14 2.14 0 01.57-1.49 2 2 0 01.05-1.47s.47-.15 1.53.57a5.27 5.27 0 012.78 0c1.06-.72 1.53-.57 1.53-.57a2 2 0 01.05 1.47 2.14 2.14 0 01.57 1.49c0 2.13-1.3 2.6-2.53 2.74a1.32 1.32 0 01.38 1.03v1.53c0 .14.1.31.38.26A5.5 5.5 0 008 2.5z"
        fill="#fff"
      />
    </svg>
  );
}

// ─── CI Platform generic ─────────────────────────────────────────
function CiIcon({ color, label }: { color: string; label: string }) {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <circle cx="8" cy="8" r="6" fill={color} opacity="0.15" />
      <circle cx="8" cy="8" r="6" fill="none" stroke={color} strokeWidth="1" />
      <text
        x="8"
        y="10.5"
        textAnchor="middle"
        fontSize="6"
        fontWeight="700"
        fontFamily="system-ui"
        fill={color}
      >
        {label}
      </text>
    </svg>
  );
}

// ─── Dockerfile ──────────────────────────────────────────────────
function DockerfileIcon() {
  return DockerIcon();
}

// ─── Turbo / Nx ──────────────────────────────────────────────────
function TurboIcon() {
  return <LabelIcon color="#EF4444" label="T" />;
}

function NxIcon() {
  return <LabelIcon color="#143055" label="Nx" />;
}

// ─── Deno ────────────────────────────────────────────────────────
function DenoIcon() {
  return <LabelIcon color="#000" label="Dn" />;
}

// ─── Just / Task ─────────────────────────────────────────────────
function JustIcon() {
  return <LabelIcon color="#8B5CF6" label="just" />;
}

function TaskIcon() {
  return <LabelIcon color="#29BEB0" label="task" />;
}

// ─── Procfile ────────────────────────────────────────────────────
function ProcIcon() {
  return <LabelIcon color="#6C4AB6" label="P" />;
}

// ─── Meson ───────────────────────────────────────────────────────
function MesonIcon() {
  return <LabelIcon color="#007EC6" label="M" />;
}

// ─── Vagrant ─────────────────────────────────────────────────────
function VagrantIcon() {
  return <LabelIcon color="#1868F2" label="V" />;
}

// ─── Skaffold ────────────────────────────────────────────────────
function SkaffoldIcon() {
  return <LabelIcon color="#2196F3" label="Sk" />;
}

// ─── Netlify / Vercel ────────────────────────────────────────────
function NetlifyIcon() {
  return <LabelIcon color="#00C7B7" label="NF" />;
}

function VercelIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <polygon points="8,2 15,14 1,14" fill="#e1e4ef" />
    </svg>
  );
}

// ─── Pre-commit ──────────────────────────────────────────────────
function PreCommitIcon() {
  return <LabelIcon color="#FAB040" label="pre" />;
}

// ─── Grunt / Gulp ────────────────────────────────────────────────
function GruntIcon() {
  return <LabelIcon color="#FAA918" label="G!" />;
}

function GulpIcon() {
  return <LabelIcon color="#CF4647" label="glp" />;
}

// ─── Vitest ─────────────────────────────────────────────────────
function VitestIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <path d="M8 1L1 8l3 7h8l3-7L8 1z" fill="#729B1B" opacity="0.15" />
      <path
        d="M5 5l3 6 3-6"
        stroke="#729B1B"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <path
        d="M4 4l4-2 4 2"
        stroke="#FCC72B"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}

// ─── Jest ────────────────────────────────────────────────────────
function JestIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <circle cx="8" cy="11" r="3.5" fill="none" stroke="#C21325" strokeWidth="1.2" />
      <circle cx="8" cy="11" r="1.2" fill="#C21325" />
      <path d="M5.5 2h5L8 8.5 5.5 2z" fill="#C21325" />
    </svg>
  );
}

// ─── TypeScript / tsconfig ──────────────────────────────────────
function TsConfigIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#3178C6" />
      <text
        x="8"
        y="12"
        textAnchor="middle"
        fontSize="9"
        fontWeight="700"
        fontFamily="system-ui, sans-serif"
        fill="#fff"
      >
        TS
      </text>
    </svg>
  );
}

// ─── Vite ────────────────────────────────────────────────────────
function ViteIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <path
        d="M14 2L8 14 2 2"
        stroke="#646CFF"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <path d="M10 1l-2 6" stroke="#FFBD2E" strokeWidth="1.5" strokeLinecap="round" fill="none" />
    </svg>
  );
}

// ─── Webpack ─────────────────────────────────────────────────────
function WebpackIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <polygon
        points="8,1 14,4.5 14,11.5 8,15 2,11.5 2,4.5"
        fill="none"
        stroke="#8DD6F9"
        strokeWidth="1"
      />
      <polygon points="8,4 11,6 11,10 8,12 5,10 5,6" fill="#1C78C0" opacity="0.6" />
    </svg>
  );
}

// ─── Tauri ───────────────────────────────────────────────────────
function TauriIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <circle cx="6" cy="6" r="3.5" fill="none" stroke="#FFC131" strokeWidth="1.2" />
      <circle cx="6" cy="6" r="1.3" fill="#FFC131" />
      <circle cx="10" cy="10" r="3.5" fill="none" stroke="#24C8DB" strokeWidth="1.2" />
      <circle cx="10" cy="10" r="1.3" fill="#24C8DB" />
    </svg>
  );
}

// ─── ESLint ──────────────────────────────────────────────────────
function EslintIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <polygon points="8,1 15,5 15,11 8,15 1,11 1,5" fill="#4B32C3" opacity="0.15" />
      <polygon points="8,3 13,6 13,10 8,13 3,10 3,6" fill="none" stroke="#4B32C3" strokeWidth="1" />
      <polygon points="8,5.5 10.5,7 10.5,9.5 8,11 5.5,9.5 5.5,7" fill="#4B32C3" opacity="0.5" />
    </svg>
  );
}

// ─── Prettier ────────────────────────────────────────────────────
function PrettierIcon() {
  return <LabelIcon color="#F7B93E" label="fmt" />;
}

// ─── Biome ──────────────────────────────────────────────────────
function BiomeIcon() {
  return <LabelIcon color="#60A5FA" label="bio" />;
}

// ─── Chibby ─────────────────────────────────────────────────────
function ChibbyIcon() {
  return <LabelIcon color="#6c8aff" label="CB" />;
}

// ─── Generic file icon ───────────────────────────────────────────
function GenericFileIcon() {
  return (
    <svg width={S} height={S} viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <path d="M3 1h7l3 3v11H3V1z" fill="none" stroke="#8b8fa7" strokeWidth="1" />
      <path d="M10 1v3h3" fill="none" stroke="#8b8fa7" strokeWidth="1" />
    </svg>
  );
}

// ─── Main export ─────────────────────────────────────────────────

export function FileTypeIcon({ scriptType }: { scriptType: string }) {
  switch (scriptType) {
    // Node / JS
    case 'PackageJson':
      return <NpmIcon />;
    case 'Turborepo':
      return <TurboIcon />;
    case 'Nx':
      return <NxIcon />;
    case 'Deno':
      return <DenoIcon />;
    case 'Grunt':
      return <GruntIcon />;
    case 'Gulp':
      return <GulpIcon />;
    // Rust
    case 'CargoToml':
      return <RustIcon />;
    // Go
    case 'GoMod':
      return <GoIcon />;
    // Python
    case 'PythonProject':
    case 'PythonRequirements':
    case 'Tox':
      return <PythonIcon />;
    case 'Pytest':
    case 'PythonTestDir':
      return <LabelIcon color="#009FE3" label="test" />;
    // Ruby
    case 'Gemfile':
    case 'Rakefile':
      return <RubyIcon />;
    // Java
    case 'Maven':
      return <MavenIcon />;
    case 'Gradle':
      return <GradleIcon />;
    // .NET
    case 'DotNet':
      return <DotNetIcon />;
    // PHP
    case 'Composer':
      return <PhpIcon />;
    // C / C++
    case 'CMake':
      return <CMakeIcon />;
    case 'Meson':
      return <MesonIcon />;
    // Make / tasks
    case 'Makefile':
      return <MakeIcon />;
    case 'Justfile':
      return <JustIcon />;
    case 'Taskfile':
      return <TaskIcon />;
    // Shell
    case 'ShellScript':
      return <ShellIcon />;
    // Env
    case 'EnvFile':
      return <EnvIcon />;
    // Containers
    case 'Dockerfile':
      return <DockerfileIcon />;
    case 'DockerCompose':
      return <DockerIcon />;
    case 'Skaffold':
      return <SkaffoldIcon />;
    case 'Vagrantfile':
      return <VagrantIcon />;
    // CI platforms
    case 'GithubActions':
      return <GitHubIcon />;
    case 'GitlabCi':
      return <CiIcon color="#FC6D26" label="GL" />;
    case 'Jenkinsfile':
      return <CiIcon color="#D33833" label="J" />;
    case 'TravisCi':
      return <CiIcon color="#3EAAAF" label="T" />;
    case 'DroneCi':
      return <CiIcon color="#212121" label="Dr" />;
    case 'CircleCi':
      return <CiIcon color="#343434" label="CI" />;
    case 'AzurePipelines':
      return <CiIcon color="#0078D4" label="Az" />;
    case 'BitbucketPipelines':
      return <CiIcon color="#0052CC" label="BB" />;
    // Deploy
    case 'Netlify':
      return <NetlifyIcon />;
    case 'Vercel':
      return <VercelIcon />;
    // Quality
    case 'PreCommit':
      return <PreCommitIcon />;
    // Process
    case 'Procfile':
      return <ProcIcon />;
    // Test frameworks
    case 'Vitest':
      return <VitestIcon />;
    case 'Jest':
      return <JestIcon />;
    case 'Mocha':
      return <LabelIcon color="#8D6748" label="mch" />;
    // TypeScript
    case 'TsConfig':
      return <TsConfigIcon />;
    // Bundlers
    case 'ViteConfig':
      return <ViteIcon />;
    case 'WebpackConfig':
      return <WebpackIcon />;
    // Tauri
    case 'TauriConfig':
      return <TauriIcon />;
    // Linters / formatters
    case 'Eslint':
      return <EslintIcon />;
    case 'Prettier':
      return <PrettierIcon />;
    case 'Biome':
      return <BiomeIcon />;
    // Chibby
    case 'ChibbyConfig':
      return <ChibbyIcon />;
    default:
      return <GenericFileIcon />;
  }
}
