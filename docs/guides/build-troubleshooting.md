# Build Troubleshooting Guide

This guide covers common build issues when building Chibby for production.

## Prerequisites

Before building, ensure you have:

- Node.js v18 or higher
- Rust (latest stable)
- Platform-specific dependencies (see main Contributing guide)

## Running a Production Build

```bash
cd chibby
npm run tauri build
```

For macOS app bundle only (recommended):
```bash
npm run tauri build -- --bundles app
```

## Common Issues

### 1. Bundle Identifier Conflicts (macOS)

**Error:**
```
Warn The bundle identifier "com.example.app" ends with `.app`. This is not recommended.
```

**Cause:** The identifier in `tauri.conf.json` ends with `.app`, which conflicts with the macOS application bundle extension.

**Solution:** Change the identifier to not end with `.app`:

```json
{
  "identifier": "com.example.dev"  // Good
  // NOT: "com.example.app"        // Bad - conflicts with .app extension
}
```

### 2. DMG Bundling Fails on macOS

**Error:**
```
failed to bundle project error running bundle_dmg.sh
Not enough arguments. Run 'create-dmg --help' for help.
```

**Cause:** The `create-dmg` tool is not installed or there's a bundling script issue.

**Solutions:**

**Option A:** Build only the .app bundle (recommended for development):
```bash
npm run tauri build -- --bundles app
```

**Option B:** Install create-dmg for DMG creation:
```bash
brew install create-dmg
```

**Option C:** Configure tauri.conf.json to skip DMG:
```json
{
  "bundle": {
    "targets": ["app"]  // Instead of "all"
  }
}
```

### 3. Code Signing Issues (macOS)

**Error:**
```
error: The application couldn't be signed.
```

**Solution:** For local development builds, you can skip signing. For distribution, you need an Apple Developer certificate. See the [Apple Developer Documentation](https://developer.apple.com/documentation/xcode/notarizing_macos_software_before_distribution).

### 4. Frontend Build Fails

**Error:**
```
error during build:
Error: Build failed
```

**Solution:** Ensure frontend builds successfully first:
```bash
npm run build        # Build frontend
npm run type-check   # Check TypeScript
npm run lint         # Check for linting errors
```

### 5. Rust Compilation Errors

**Error:**
```
error[E0XXX]: ...
```

**Solution:** 
```bash
cd src-tauri
cargo check          # Check for errors
cargo clean          # Clean build artifacts if needed
cargo build --release
```

## Build Output Locations

After a successful build:

| Platform | Location |
|----------|----------|
| macOS    | `src-tauri/target/release/bundle/macos/Chibby.app` |
| Windows  | `src-tauri/target/release/bundle/msi/` |
| Linux    | `src-tauri/target/release/bundle/deb/` or `appimage/` |

## Verifying the Build

```bash
# macOS - open the app directly
open src-tauri/target/release/bundle/macos/Chibby.app

# Or run the binary
./src-tauri/target/release/chibby
```

## Getting Help

If you encounter issues not covered here:

1. Check the [Tauri documentation](https://tauri.app/v1/guides/)
2. Search existing issues in the repository
3. Create a new issue with build logs and system info
