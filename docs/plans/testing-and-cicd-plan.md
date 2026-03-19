# Chibby Testing and CI/CD Plan

Status: Draft
Created: 2026-03-17

## Purpose

This document outlines the testing strategy and CI/CD setup for Chibby itself.
Since Chibby is a CI/CD tool, we will dogfood it by using Chibby to manage its
own build and test pipeline alongside traditional GitHub Actions.

## Testing Philosophy

- **Test what matters**: Focus on business logic, not implementation details
- **Fast feedback**: Unit tests should run in under 5 seconds
- **Confidence over coverage**: Prioritize tests that catch real bugs
- **Cross-platform**: Tests must pass on macOS, Linux, and Windows

---

## Part 1: Frontend Testing (React + TypeScript)

### Test Stack

| Tool | Purpose |
|------|---------|
| Vitest | Test runner (already in devDependencies) |
| @testing-library/react | Component testing |
| @testing-library/user-event | User interaction simulation |
| happy-dom | Fast DOM implementation |
| msw (Mock Service Worker) | API mocking for Tauri commands |

### Installation

```bash
npm install -D @testing-library/react @testing-library/user-event @testing-library/jest-dom happy-dom msw @vitest/coverage-v8
```

### Test Structure

```
frontend/
  __tests__/              # Test files mirror src structure
    components/
      ProjectList.test.tsx
      ProjectDetail.test.tsx
      PipelineEditor.test.tsx
      ...
    hooks/
      useProjects.test.ts
      usePipeline.test.ts
    utils/
      format.test.ts
    services/
      api.test.ts         # Tests API function behavior with mocks
  __mocks__/
    tauri.ts              # Mock Tauri invoke
  test-utils.tsx          # Custom render with providers
```

### Test Categories

#### 1. Unit Tests (utils, helpers, pure functions)

Priority: High
Location: `frontend/__tests__/utils/`

| File | What to Test |
|------|--------------|
| `format.ts` | Date formatting, duration formatting, status classes |
| Type validators | Pipeline/Stage validation helpers |

Example:

```typescript
// frontend/__tests__/utils/format.test.ts
import { describe, it, expect } from 'vitest';
import { formatDuration, statusClass, capitalize } from '../../utils/format';

describe('formatDuration', () => {
  it('formats milliseconds to human readable', () => {
    expect(formatDuration(1500)).toBe('1.5s');
    expect(formatDuration(65000)).toBe('1m 5s');
    expect(formatDuration(undefined)).toBe('-');
  });
});

describe('statusClass', () => {
  it('returns correct CSS class for status', () => {
    expect(statusClass('success')).toBe('success');
    expect(statusClass('failed')).toBe('failed');
    expect(statusClass('running')).toBe('running');
  });
});
```

#### 2. Hook Tests

Priority: Medium
Location: `frontend/__tests__/hooks/`

Test custom hooks in isolation using `@testing-library/react`.

| Hook | What to Test |
|------|--------------|
| `useProjects` (if exists) | Loading states, error handling, data fetching |
| `usePipeline` (if exists) | Pipeline CRUD operations |

#### 3. Component Tests

Priority: High
Location: `frontend/__tests__/components/`

Focus on user interactions and rendered output, not implementation.

| Component | What to Test |
|-----------|--------------|
| `ProjectList` | Renders projects, handles empty state, add project flow |
| `ProjectDetail` | Shows pipeline stages, run history, validation warnings |
| `PipelineEditor` | Add/remove stages, import from CI, save pipeline |
| `RunDetail` | Log display, stage status indicators |
| `EnvironmentEditor` | Add/edit environments, variable management |
| `SecretsManager` | Secret status display, add/delete secrets |

Example:

```typescript
// frontend/__tests__/components/ProjectList.test.tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';
import ProjectList from '../../components/ProjectList';
import * as api from '../../services/api';

vi.mock('../../services/api');

describe('ProjectList', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('shows empty state when no projects', async () => {
    vi.mocked(api.listProjects).mockResolvedValue([]);
    
    render(
      <BrowserRouter>
        <ProjectList />
      </BrowserRouter>
    );
    
    expect(await screen.findByText(/no projects/i)).toBeInTheDocument();
  });

  it('renders project list', async () => {
    vi.mocked(api.listProjects).mockResolvedValue([
      {
        project: { id: '1', name: 'My App', path: '/path', added_at: '2026-01-01' },
        has_pipeline: true,
      },
    ]);
    
    render(
      <BrowserRouter>
        <ProjectList />
      </BrowserRouter>
    );
    
    expect(await screen.findByText('My App')).toBeInTheDocument();
  });
});
```

#### 4. API Mock Setup

Create a Tauri invoke mock:

```typescript
// frontend/__mocks__/tauri.ts
import { vi } from 'vitest';

export const invoke = vi.fn();

// Mock Tauri API module
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
```

### Vitest Configuration

```typescript
// vitest.config.ts (update existing)
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./frontend/test-setup.ts'],
    include: ['frontend/__tests__/**/*.test.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: ['frontend/**/*.{ts,tsx}'],
      exclude: ['frontend/__tests__/**', 'frontend/__mocks__/**'],
    },
  },
});
```

### Test Setup File

```typescript
// frontend/test-setup.ts
import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// Mock Tauri
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));
```

---

## Part 2: Backend Testing (Rust)

### Test Structure

```
src-tauri/
  src/
    engine/
      mod.rs
      detector.rs       # Unit tests inline or in tests/
      executor.rs
      pipeline.rs
      models.rs
    commands/
      mod.rs
      pipeline_commands.rs
      project_commands.rs
      run_commands.rs
      env_commands.rs
  tests/                 # Integration tests
    pipeline_test.rs
    executor_test.rs
    detector_test.rs
```

### Test Categories

#### 1. Unit Tests (inline with `#[cfg(test)]`)

Priority: High

| Module | What to Test |
|--------|--------------|
| `engine/models.rs` | Serialization/deserialization of Pipeline, Stage, Run |
| `engine/detector.rs` | Script detection, pipeline generation, validation |
| `engine/pipeline.rs` | TOML save/load, pipeline parsing |
| `engine/executor.rs` | Command execution (mock shell) |

Example:

```rust
// src/engine/detector.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_detect_package_json() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"build": "npm run build", "test": "vitest"}}"#,
        ).unwrap();

        let scripts = detect_scripts(temp.path());
        assert!(scripts.iter().any(|s| s.file_name == "package.json"));
    }

    #[test]
    fn test_validate_pipeline_missing_npm_script() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"build": "vite build"}}"#,
        ).unwrap();

        let pipeline = Pipeline {
            name: "test".to_string(),
            stages: vec![Stage {
                name: "test".to_string(),
                commands: vec!["npm test".to_string()],
                backend: Backend::Local,
                working_dir: None,
                fail_fast: true,
                health_check: None,
            }],
        };

        let validation = validate_pipeline(&pipeline, temp.path());
        assert!(!validation.is_valid);
        assert!(validation.warnings.iter().any(|w| w.command.contains("npm test")));
    }

    #[test]
    fn test_parse_github_workflows() {
        let temp = TempDir::new().unwrap();
        let workflows_dir = temp.path().join(".github/workflows");
        fs::create_dir_all(&workflows_dir).unwrap();
        fs::write(
            workflows_dir.join("ci.yml"),
            r#"
name: CI
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - name: Run tests
        run: npm test
"#,
        ).unwrap();

        let workflows = parse_github_workflows(temp.path());
        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].name, "CI");
    }
}
```

#### 2. Integration Tests

Priority: Medium
Location: `src-tauri/tests/`

Test complete workflows with real file I/O but mocked external services.

```rust
// tests/pipeline_integration.rs
use chibby_lib::engine::{pipeline, detector};
use tempfile::TempDir;
use std::fs;

#[test]
fn test_full_pipeline_cycle() {
    let temp = TempDir::new().unwrap();
    
    // Create a minimal project
    fs::write(temp.path().join("package.json"), r#"{"scripts": {"build": "echo ok"}}"#).unwrap();
    
    // Detect scripts and generate pipeline
    let scripts = detector::detect_scripts(temp.path());
    let pipeline = detector::generate_draft_pipeline("test-project", &scripts);
    
    // Save pipeline
    pipeline::save_pipeline(temp.path(), &pipeline).unwrap();
    
    // Load and verify
    let loaded = pipeline::load_pipeline(temp.path()).unwrap();
    assert_eq!(loaded.name, "test-project");
}
```

#### 3. Command Tests

Test Tauri commands as regular functions (they're just wrapped functions).

```rust
// tests/commands_test.rs
use chibby_lib::commands::pipeline_commands;

#[test]
fn test_detect_scripts_command() {
    // Commands can be tested directly since they're just Rust functions
    let result = pipeline_commands::detect_scripts("/nonexistent".to_string());
    assert!(result.is_ok()); // Returns empty list for nonexistent path
}
```

### Adding Test Dependencies

```toml
# Cargo.toml
[dev-dependencies]
tempfile = "3"
mockall = "0.13"  # For mocking traits
```

### Running Tests

```bash
cd src-tauri
cargo test                    # Run all tests
cargo test --lib              # Unit tests only
cargo test --test '*'         # Integration tests only
cargo test -- --nocapture     # Show println output
```

---

## Part 3: CI/CD Setup (GitHub Actions)

### Workflow Structure

```
.github/
  workflows/
    ci.yml              # Run on every push/PR
    release.yml         # Build and publish releases
    nightly.yml         # Optional: nightly builds
```

### CI Workflow (ci.yml)

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: chibby/package-lock.json
      
      - name: Install frontend dependencies
        working-directory: chibby
        run: npm ci
      
      - name: Lint TypeScript
        working-directory: chibby
        run: npm run lint
      
      - name: Check formatting
        working-directory: chibby
        run: npm run format:check
      
      - name: Type check
        working-directory: chibby
        run: npm run type-check

  rust-lint:
    name: Rust Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: chibby/src-tauri
      
      - name: Check formatting
        working-directory: chibby/src-tauri
        run: cargo fmt --check
      
      - name: Clippy
        working-directory: chibby/src-tauri
        run: cargo clippy -- -D warnings

  test-frontend:
    name: Frontend Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: chibby/package-lock.json
      
      - name: Install dependencies
        working-directory: chibby
        run: npm ci
      
      - name: Run tests
        working-directory: chibby
        run: npm test -- --coverage
      
      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: chibby/coverage/lcov.info
          flags: frontend

  test-backend:
    name: Backend Tests
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: chibby/src-tauri
      
      - name: Install Linux dependencies
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
      
      - name: Run tests
        working-directory: chibby/src-tauri
        run: cargo test

  build:
    name: Build Check
    needs: [lint, rust-lint, test-frontend, test-backend]
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: chibby/package-lock.json
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: chibby/src-tauri
      
      - name: Install Linux dependencies
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
      
      - name: Install frontend dependencies
        working-directory: chibby
        run: npm ci
      
      - name: Build Tauri app
        working-directory: chibby
        run: npm run tauri:build
```

---

## Part 4: Dogfooding with Chibby

### Chibby's Own Pipeline

Create `.chibby/pipeline.toml` in the chibby folder:

```toml
name = "Chibby CI"

[[stages]]
name = "lint"
commands = [
  "npm run lint",
  "npm run format:check",
  "npm run type-check"
]
backend = "local"
fail_fast = true

[[stages]]
name = "rust-lint"
commands = [
  "cd src-tauri && cargo fmt --check",
  "cd src-tauri && cargo clippy -- -D warnings"
]
backend = "local"
fail_fast = true

[[stages]]
name = "test-frontend"
commands = ["npm test"]
backend = "local"
fail_fast = true

[[stages]]
name = "test-backend"
commands = ["cd src-tauri && cargo test"]
backend = "local"
fail_fast = true

[[stages]]
name = "build"
commands = ["npm run tauri:build"]
backend = "local"
fail_fast = true
```

### Self-Testing Workflow

1. **Development**: Run Chibby locally to test itself
2. **PR Validation**: GitHub Actions runs tests (standard CI)
3. **Comparison**: Verify Chibby pipeline produces same results as GH Actions
4. **Confidence**: Once validated, Chibby can be primary test runner for local dev

---

## Part 5: Implementation Roadmap

### Phase A: Setup Testing Infrastructure (1-2 days)

- [ ] Install frontend test dependencies
- [ ] Create vitest config and test setup
- [ ] Add first unit tests for `utils/format.ts`
- [ ] Add `cargo test` with first inline unit tests
- [ ] Verify both run: `npm test` and `cargo test`

### Phase B: Core Test Coverage (3-5 days)

- [ ] Frontend: Test all utility functions
- [ ] Frontend: Test key components (ProjectList, ProjectDetail, PipelineEditor)
- [ ] Frontend: Create API mocking strategy
- [ ] Backend: Test `detector.rs` functions
- [ ] Backend: Test `pipeline.rs` TOML parsing
- [ ] Backend: Test validation logic

### Phase C: CI/CD Setup (1 day)

- [ ] Create `.github/workflows/ci.yml`
- [ ] Verify workflow runs on all platforms
- [ ] Add coverage reporting
- [ ] Add status badge to README

### Phase D: Dogfooding (ongoing)

- [ ] Create Chibby's own `.chibby/pipeline.toml`
- [ ] Use Chibby to run its own tests locally
- [ ] Compare results with GitHub Actions
- [ ] Document any gaps or improvements needed

---

## NPM Scripts Update

Add to `package.json`:

```json
{
  "scripts": {
    "test": "vitest",
    "test:run": "vitest run",
    "test:coverage": "vitest run --coverage",
    "test:ui": "vitest --ui",
    "test:all": "npm run test:run && cd src-tauri && cargo test"
  }
}
```

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Frontend test coverage | >70% for utils, >50% for components |
| Backend test coverage | >70% for engine modules |
| CI run time | <10 minutes |
| Tests pass rate | 100% on all platforms |
| Flaky test rate | <1% |

---

## Notes

- **Don't over-test**: Focus on behavior, not implementation
- **Skip mocking hell**: Test real integrations where practical
- **Prioritize debugging**: Good error messages > high coverage numbers
- **Keep tests fast**: Slow tests get skipped

## Related Documents

- [chibby-phased-build-roadmap.md](chibby-phased-build-roadmap.md) - Product roadmap
- [phase-0-audit.md](../phase-0-audit.md) - Initial workflow audit
