# Homebrew Distribution

This directory contains Homebrew formulas for distributing Chibby.

## Formulas

| Formula | Description | Install Command |
|---------|-------------|-----------------|
| `chibby.rb` | GUI app with bundled CLI | `brew install --cask chibby` |
| `chibby-cli.rb` | CLI only (no GUI) | `brew install chibby-cli` |

## Automated Deployment with Chibby

Use Chibby itself to update the Homebrew tap after a release:

```bash
# 1. Clone your homebrew tap (one-time setup)
git clone git@github.com:chibby-app/homebrew-chibby.git ../homebrew-chibby

# 2. Create a release first
chibby run --env release

# 3. Update Homebrew tap
chibby run --env homebrew

# Or set custom tap path
HOMEBREW_TAP_PATH=~/my-tap chibby run --env homebrew
```

**What happens:**
1. Downloads release artifacts from GitHub
2. Computes SHA256 hashes
3. Updates formulas with new version and hashes
4. Commits and pushes to tap repository

## Setting Up a Homebrew Tap

1. Create a new repository named `homebrew-chibby` (or `homebrew-tap`)
2. Copy the formula files to the repository:
   ```
   homebrew-chibby/
     Casks/
       chibby.rb
     Formula/
       chibby-cli.rb
   ```
3. Users can then install with:
   ```bash
   brew tap chibby-app/chibby
   brew install --cask chibby      # GUI + CLI
   # or
   brew install chibby-app/chibby/chibby-cli  # CLI only
   ```

## Updating Formulas for a Release

After creating a GitHub release:

1. Download the release artifacts
2. Calculate SHA256 hashes:
   ```bash
   shasum -a 256 Chibby_*.dmg
   shasum -a 256 chibby-cli-*.tar.gz
   ```
3. Update the formula files with new version and hashes
4. Commit and push to the tap repository

## Automated Updates

Consider using [homebrew-releaser](https://github.com/Justintime50/homebrew-releaser) 
or a GitHub Action to automatically update formulas when releases are published.

Example workflow addition for `.github/workflows/release.yml`:

```yaml
update-homebrew:
  name: Update Homebrew Tap
  needs: [publish-release]
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        repository: chibby-app/homebrew-chibby
        token: ${{ secrets.HOMEBREW_TAP_TOKEN }}

    # Calculate hashes and update formulas
    # ... formula update logic ...

    - name: Commit and push
      run: |
        git config user.name "github-actions[bot]"
        git config user.email "github-actions[bot]@users.noreply.github.com"
        git add .
        git commit -m "Update to v${{ github.ref_name }}"
        git push
```

## User Installation

### Full App (GUI + CLI)

```bash
brew tap chibby-app/chibby
brew install --cask chibby
```

This installs:
- Chibby.app in /Applications
- `chibby` command symlinked to /usr/local/bin

### CLI Only

For headless servers or terminal-only workflows:

```bash
brew tap chibby-app/chibby
brew install chibby-app/chibby/chibby-cli
```

This installs only the `chibby` command.

## What Users Get

| Installation | GUI App | CLI Command |
|--------------|---------|-------------|
| `brew install --cask chibby` | Yes | Yes |
| `brew install chibby-cli` | No | Yes |

Both installations provide the same `chibby` command with identical functionality.
Data is stored in the same location (`~/Library/Application Support/chibby`),
so users can switch between GUI and CLI seamlessly.
