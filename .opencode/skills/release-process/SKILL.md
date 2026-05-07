---
name: release-process
description: Create a new release: version bump, tarball creation, git tagging, and GitHub release with attached binaries.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: release
---

## What I do

I guide the complete release process for sprachspiel: bumping version numbers, updating documentation, creating tarballs, tagging, and publishing the GitHub release.

## When to use me

Use this skill when the user asks to create a release or when all planned features for a version are complete and ready to ship.

## Step 1: Update Version Numbers

Update the version `X.Y.Z` in three files:

### 1.1 `Cargo.toml`

```toml
version = "X.Y.Z"
```

### 1.2 `man/sprachspiel.1`

Update the `.TH` line with the new version number:

```
.TH "SPRACHSPIEL" "1" "April 2026" "X.Y.Z" "Sprachspiel Manual"
```

### 1.3 `doc/src/CHANGELOG.md`

Add a new version section at the top (after `[Unreleased]`):

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added

- Feature description

### Changed

- Change description

### Fixed

- Fix description

### Removed

- Removal description
```

## Step 2: Update CHANGELOG

Collect all changes since the last release:

1. Review `git log <last-tag>..HEAD --oneline`
2. Categorize changes into Added/Changed/Fixed/Removed
3. Write clear, user-facing descriptions (not internal details)

## Step 3: Commit and Push

```bash
git add Cargo.toml Cargo.lock man/sprachspiel.1 doc/src/CHANGELOG.md
git commit -m "chore: bump version to X.Y.Z"
git push origin master
```

## Step 4: Create Tarballs

```bash
make all-tarballs
```

This creates 4 tarballs in `dist/`:

| Tarball | Platform | Features |
|---------|----------|----------|
| `sprachspiel-X.Y.Z-linux-x86_64.tar.gz` | Linux x86_64 | Default features |
| `sprachspiel-X.Y.Z-linux-x86_64-all-tools.tar.gz` | Linux x86_64 | All features |
| `sprachspiel-X.Y.Z-termux-aarch64-linux-android.tar.gz` | Termux (Android) | Default features |
| `sprachspiel-X.Y.Z-termux-aarch64-linux-android-all-tools.tar.gz` | Termux (Android) | All features |

### Tarball Contents

- **Linux**: Binary, man page (`sprachspiel.1`), `README.md`, `LICENSE.txt`
- **Termux**: Binary, `README-TERMUX.txt` with installation instructions

## Step 5: Create Tag and GitHub Release

```bash
# Create annotated tag
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z

# Create GitHub release with all 4 tarballs
gh release create vX.Y.Z \
  --title "vX.Y.Z" \
  --notes "## Changes

$(head -50 doc/src/CHANGELOG.md)" \
  dist/sprachspiel-X.Y.Z-linux-x86_64.tar.gz \
  dist/sprachspiel-X.Y.Z-linux-x86_64-all-tools.tar.gz \
  dist/sprachspiel-X.Y.Z-termux-aarch64-linux-android.tar.gz \
  dist/sprachspiel-X.Y.Z-termux-aarch64-linux-android-all-tools.tar.gz
```

## Step 6: Post-Release Verification

1. Check the release page on GitHub: all 4 tarballs attached
2. Verify tarball contents: binary runs, man page present
3. Update `IMPLEMENTATION.md` version header if needed
4. Verify project board: all issues in the release should have cards in "Done"
5. Check for any stale cards (issues that are closed but cards still in "In Progress" or "In Review")