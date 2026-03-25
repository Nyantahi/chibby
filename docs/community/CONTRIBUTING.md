# Contributing to Chibby

Thanks for your interest in contributing to Chibby! This guide will help you get started.

## Development Setup

### Prerequisites

- [Node.js 22+](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS

### Getting Started

```bash
# Clone the repo
git clone https://github.com/Nyantahi/chibby.git
cd chibby

# Install dependencies
npm install

# Run in development mode
npm run tauri:dev
```

### Project Structure

```
chibby/
├── frontend/          # React TypeScript UI
│   ├── components/    # React components
│   ├── services/      # API layer (Tauri invoke wrappers)
│   ├── styles/        # CSS
│   └── types/         # TypeScript types
├── src-tauri/         # Rust backend
│   └── src/
│       ├── commands/  # Tauri command handlers
│       └── engine/    # CI/CD engine (detection, pipeline, execution)
└── docs/              # Documentation and plans
```

## Development Workflow

1. Fork the repo and create a feature branch from `main`
2. Make your changes
3. Run checks before committing:

```bash
npm run type-check     # TypeScript check
npm run lint           # ESLint
npm run format:check   # Prettier
npm run test:run       # Vitest
cd src-tauri && cargo test  # Rust tests
```

4. Commit with a descriptive message (we use [Conventional Commits](https://www.conventionalcommits.org/))
5. Open a pull request against `main`

## Code Style

- **TypeScript**: Enforced by ESLint and Prettier (run `npm run lint:fix` and `npm run format`)
- **Rust**: Enforced by `cargo fmt` and `cargo clippy`

## Reporting Issues

Use the [issue templates](../../.github/ISSUE_TEMPLATE/) for bug reports and feature requests.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](../../LICENSE).
