# Okapian Deploy Workflow Audit

**Date:** 2026-03-16
**Purpose:** Document Okapian's current deploy workflows so Chibby can automate them.

---

## Project Overview

Okapian has two major deployment targets:

1. **Tauri Desktop App** — macOS/Linux/Windows builds (currently manual)
2. **Website** — Docker Compose stack deployed over SSH to VPS

---

## Target 1: Tauri Desktop App Build & Release

### Current State

- **No automated release process** — builds are triggered manually via `npm run tauri:build`
- **No code signing** — macOS builds are unsigned, Windows builds are unsigned
- **No notarization** — macOS builds are not notarized (Gatekeeper blocks unsigned apps)
- **No artifact storage** — built artifacts stay in `backend/target/release/bundle/`
- **No update distribution** — users must manually download new versions
- **No version synchronization** — `package.json` (1.0.0) vs `tauri.conf.json` (0.1.0) are mismatched

### Command Sequence

```bash
# Pre-build: Ensure clean state
cd /path/to/okapian
npm install
cd backend && cargo fetch

# Stage 1: Type-check (catches TS errors before build)
npm run type-check

# Stage 2: Lint (catch style issues)
npm run lint

# Stage 3: Test
npm test

# Stage 4: Build frontend
npm run build

# Stage 5: Build Tauri app
npm run tauri:build

# Output locations:
#   macOS:   backend/target/release/bundle/macos/okapian.app
#            backend/target/release/bundle/dmg/okapian_*.dmg
#   Linux:   backend/target/release/bundle/deb/*.deb
#            backend/target/release/bundle/appimage/*.AppImage
#   Windows: backend/target/release/bundle/msi/*.msi
#            backend/target/release/bundle/nsis/*.exe
```

### macOS Signing & Notarization (not yet implemented)

```bash
# Requires Apple Developer credentials:
# - APPLE_SIGNING_IDENTITY (Developer ID Application: ...)
# - APPLE_ID (for notarization)
# - APPLE_TEAM_ID
# - APPLE_PASSWORD (app-specific password)

# Sign the app bundle
codesign --deep --force --options runtime \
  --sign "$APPLE_SIGNING_IDENTITY" \
  backend/target/release/bundle/macos/okapian.app

# Create signed DMG
create-dmg --volname "Okapian" \
  --codesign "$APPLE_SIGNING_IDENTITY" \
  okapian.dmg \
  backend/target/release/bundle/macos/okapian.app

# Notarize (async)
xcrun notarytool submit okapian.dmg \
  --apple-id "$APPLE_ID" \
  --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_PASSWORD" \
  --wait

# Staple notarization ticket
xcrun stapler staple okapian.dmg
```

### Windows Signing (not yet implemented)

```bash
# Requires:
# - Windows code signing certificate (.pfx or hardware token)
# - signtool.exe (from Windows SDK)

signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 \
  /f certificate.pfx /p "$CERT_PASSWORD" \
  backend/target/release/bundle/nsis/okapian-setup.exe
```

### Secrets Required

| Secret | Purpose | Storage |
|--------|---------|---------|
| `APPLE_SIGNING_IDENTITY` | macOS code signing | Keychain |
| `APPLE_ID` | Apple notarization | Keychain |
| `APPLE_TEAM_ID` | Apple team identifier | Plain config |
| `APPLE_PASSWORD` | App-specific password | Keychain |
| `WINDOWS_CERT_PFX` | Windows code signing cert | File + password |
| `WINDOWS_CERT_PASSWORD` | PFX password | Keychain |

### Pipeline Definition (Chibby)

```toml
# .chibby/pipeline.toml

name = "Okapian Desktop Release"

[[stages]]
name = "install"
commands = ["npm install"]
backend = "local"
fail_fast = true

[[stages]]
name = "type-check"
commands = ["npm run type-check"]
backend = "local"
fail_fast = true

[[stages]]
name = "lint"
commands = ["npm run lint"]
backend = "local"
fail_fast = true

[[stages]]
name = "test"
commands = ["npm test"]
backend = "local"
fail_fast = true

[[stages]]
name = "build"
commands = ["npm run tauri:build"]
backend = "local"
fail_fast = true

# macOS-only stages (conditional on platform)
[[stages]]
name = "sign-macos"
commands = [
  "codesign --deep --force --options runtime --sign \"$APPLE_SIGNING_IDENTITY\" backend/target/release/bundle/macos/okapian.app"
]
backend = "local"
fail_fast = true

[[stages]]
name = "notarize-macos"
commands = [
  "xcrun notarytool submit backend/target/release/bundle/dmg/*.dmg --apple-id \"$APPLE_ID\" --team-id \"$APPLE_TEAM_ID\" --password \"$APPLE_PASSWORD\" --wait",
  "xcrun stapler staple backend/target/release/bundle/dmg/*.dmg"
]
backend = "local"
fail_fast = true
```

---

## Target 2: Website Docker Compose Deploy

### Current State

- **CI runs tests only** — GitHub Actions run backend + frontend tests but never deploy
- **No deploy pipeline** — manual `docker compose up` on VPS
- **Secrets in `.env` files** — not in a keychain or secret manager
- **No health checks post-deploy** — no automated verification
- **No rollback** — if deploy fails, manual intervention required

### Existing CI Workflows

Located in `website/.github/workflows/`:

| Workflow | Trigger | Purpose | Deploys? |
|----------|---------|---------|----------|
| `backend-testing.yml` | Push to `backend/**` | Lint, security scan, pytest | No |
| `frontend-risk-based-testing.yml` | Push to `frontend/**` | Critical/high-risk tests | No |
| `docs.yml` | Push to `docs/**` | Build docs | No |

### Docker Compose Services

From `website/docker-compose.yml`:

| Service | Port | Image/Build |
|---------|------|-------------|
| `backend` | 8000 | `./backend/Dockerfile` |
| `frontend` | 3000 | `./frontend/Dockerfile` |
| `admin` | 3001 | `./admin/Dockerfile` |
| `redis` | 6379 | `redis:7-alpine` |

**External dependency:** PostgreSQL (not in compose, likely managed DB)

### Command Sequence (SSH Deploy)

```bash
# Pre-deploy: Run tests locally
cd /path/to/okapian/website
npm --prefix frontend ci
npm --prefix frontend run test:pr
cd backend && pip install -r requirements-dev.txt && pytest

# Stage 1: Build images locally and push (or build on server)
docker compose build

# Stage 2: Transfer to server (option A: push to registry)
docker tag app-frontend your-registry.io/app-frontend:latest
docker push your-registry.io/app-frontend:latest
# ... repeat for backend, admin

# Stage 2 alt: Transfer to server (option B: rsync + build on server)
rsync -avz --delete \
  --exclude 'node_modules' \
  --exclude '.git' \
  --exclude 'dist' \
  ./ user@server:/app/okapian-website/

# Stage 3: SSH to server and deploy
ssh user@server << 'EOF'
cd /app/okapian-website
docker compose pull  # if using registry
docker compose up -d --build --force-recreate
docker compose ps
EOF

# Stage 4: Health check
curl -f https://your-domain.com/api/health || exit 1
curl -f https://your-domain.com/ || exit 1
```

### Secrets Required

| Secret | Purpose | Current Storage |
|--------|---------|-----------------|
| `DATABASE_URL` | PostgreSQL connection | `.env` file |
| `POSTGRES_PASSWORD` | DB password | `.env` file |
| `JWT_SECRET_KEY` | Auth tokens | `.env` file |
| `SECRET_KEY` | App secret | `.env` file |
| `STRIPE_SECRET_KEY` | Payments | `.env` file |
| `OPENAI_API_KEY` | AI features | `.env` file |
| `SSH_PRIVATE_KEY` | Server access | `~/.ssh/` |
| `SSH_HOST` | Server hostname | Plain config |
| `SSH_USER` | Server username | Plain config |

### Pipeline Definition (Chibby)

```toml
# website/.chibby/pipeline.toml

name = "Okapian Website Deploy"

[[stages]]
name = "install-frontend"
commands = ["npm --prefix frontend ci"]
backend = "local"
fail_fast = true

[[stages]]
name = "test-frontend"
commands = ["npm --prefix frontend run test:pr"]
backend = "local"
fail_fast = true

[[stages]]
name = "install-backend"
commands = ["pip install -r backend/requirements-dev.txt"]
backend = "local"
fail_fast = true

[[stages]]
name = "test-backend"
commands = ["cd backend && pytest"]
backend = "local"
fail_fast = true

[[stages]]
name = "build-images"
commands = ["docker compose build"]
backend = "local"
fail_fast = true

[[stages]]
name = "sync-to-server"
commands = [
  "rsync -avz --delete --exclude 'node_modules' --exclude '.git' --exclude 'dist' --exclude '__pycache__' ./ $SSH_USER@$SSH_HOST:/app/okapian-website/"
]
backend = "local"
fail_fast = true

[[stages]]
name = "deploy"
commands = [
  "cd /app/okapian-website && docker compose up -d --build --force-recreate",
  "docker compose ps"
]
backend = "ssh"
fail_fast = true

[[stages]]
name = "health-check"
commands = [
  "curl -f https://your-domain.com/api/health",
  "curl -f https://your-domain.com/"
]
backend = "local"
fail_fast = true
```

---

## Environment Definitions

```toml
# website/.chibby/environments.toml

[[environment]]
name = "production"
ssh_host = "your-server.com"
ssh_port = 22

[environment.variables]
APP_ENV = "production"
LOG_LEVEL = "info"

[[environment]]
name = "staging"
ssh_host = "staging.your-server.com"
ssh_port = 22

[environment.variables]
APP_ENV = "staging"
LOG_LEVEL = "debug"
```

---

## Secret References

```toml
# website/.chibby/secrets.toml
# Values stored in OS keychain, not here

[[secret]]
name = "DATABASE_URL"
environments = ["production", "staging"]

[[secret]]
name = "JWT_SECRET_KEY"
environments = ["production", "staging"]

[[secret]]
name = "STRIPE_SECRET_KEY"
environments = ["production"]

[[secret]]
name = "SSH_PRIVATE_KEY"
environments = ["production", "staging"]
```

---

## Pain Points Confirmed

1. **Version mismatch** — `package.json` says 1.0.0, `tauri.conf.json` says 0.1.0
2. **No signing** — Desktop builds are blocked by Gatekeeper
3. **CI tests only** — Website CI runs tests but never deploys
4. **Secrets in files** — `.env` files checked in or manually managed
5. **No rollback** — If deploy breaks, must manually fix
6. **No deploy history** — No record of what was deployed when
7. **No health checks** — Manual verification after deploy

---

## Chibby Must Support

To replace manual Okapian workflows, Chibby needs:

1. **Local process execution** ✅ (Phase 1 done)
2. **SSH command execution** (Phase 4)
3. **Docker Compose over SSH** (Phase 4)
4. **OS keychain secret storage** (Phase 4)
5. **Health check validation** (Phase 4)
6. **Run history** ✅ (Phase 1 done)
7. **Log streaming** ✅ (Phase 3 done)
8. **Rollback commands** (Phase 5)
9. **macOS notarization polling** (future)
10. **Cross-platform builds** (future - remote agents)

---

## Next Steps

1. Use Okapian as the first test project in Chibby
2. Test local pipeline execution with `npm run tauri:build`
3. Implement SSH backend for website deploy
4. Add keychain integration for secrets
5. Add health check stage type
