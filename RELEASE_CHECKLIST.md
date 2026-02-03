# Release Checklist

Use this checklist when creating a new release.

## Pre-Release (1 week before)

- [ ] Update version in `Cargo.toml`:
  ```bash
  # cis-core/Cargo.toml
  # cis-node/Cargo.toml
  # cis-skill-sdk/Cargo.toml
  ```
- [ ] Update `CHANGELOG.md` with release date
- [ ] Run full test suite: `cargo test --all`
- [ ] Run clippy: `cargo clippy --all -- -D warnings`
- [ ] Run security audit: `cargo audit`
- [ ] Update documentation if needed
- [ ] Create release branch: `git checkout -b release/v1.0.0`

## Pre-Release (1 day before)

- [ ] Merge release branch to main
- [ ] Tag the release: `git tag -a v1.0.0 -m "Release v1.0.0"`
- [ ] Push tag: `git push origin v1.0.0`
- [ ] Verify CI/CD pipeline starts

## Release Day

- [ ] Verify all artifacts built successfully:
  - [ ] macOS `.dmg`
  - [ ] macOS `.app.tar.gz`
  - [ ] Linux `.AppImage`
  - [ ] Linux `.deb`
  - [ ] Linux `.tar.gz`
  - [ ] Windows `.msi`
  - [ ] Windows `.zip`
- [ ] Verify GitHub Release created with:
  - [ ] All artifacts attached
  - [ ] Release notes populated
  - [ ] Correct version tag
- [ ] Test installation on each platform:
  - [ ] macOS (Intel)
  - [ ] macOS (Apple Silicon)
  - [ ] Linux (Ubuntu)
  - [ ] Linux (Fedora)
  - [ ] Windows 10/11
- [ ] Update `latest` tag: `git tag -f latest v1.0.0 && git push -f origin latest`
- [ ] Update website/documentation links
- [ ] Announce release:
  - [ ] GitHub Discussions
  - [ ] Twitter/Mastodon
  - [ ] Discord/Slack
  - [ ] Email newsletter

## Post-Release

- [ ] Monitor for issues (24-48 hours)
- [ ] Respond to user feedback
- [ ] Update installation scripts if needed
- [ ] Plan hotfix if critical issues found
- [ ] Merge main back to develop

## Hotfix Release (if needed)

- [ ] Create hotfix branch from tag: `git checkout -b hotfix/v1.0.1 v1.0.0`
- [ ] Apply fix
- [ ] Update version in `Cargo.toml`
- [ ] Update `CHANGELOG.md`
- [ ] Tag: `git tag -a v1.0.1 -m "Hotfix v1.0.1"`
- [ ] Push and verify CI/CD
- [ ] Update `latest` tag

## Version Number Format

- Stable: `v1.0.0`, `v1.0.1`, `v1.1.0`
- Pre-release: `v1.0.0-alpha.1`, `v1.0.0-beta.1`, `v1.0.0-rc.1`

## GitHub Release Template

```markdown
## What's New

### Added
- Feature 1
- Feature 2

### Fixed
- Bug fix 1
- Bug fix 2

## Installation

### macOS
Download `.dmg` and drag to Applications, or use Homebrew:
\`\`\`bash
brew install cis
\`\`\`

### Linux
Download `.AppImage` (portable) or install `.deb`:
\`\`\`bash
sudo dpkg -i cis_1.0.0_amd64.deb
\`\`\`

### Windows
Download and run `.msi` installer, or use winget:
\`\`\`powershell
winget install CIS
\`\`\`

## Verification

Verify checksums:
\`\`\`bash
sha256sum -c cis-1.0.0-SHA256SUMS
\`\`\`

## Full Changelog

See [CHANGELOG.md](../blob/main/CHANGELOG.md)
```

## Automation

Most of these steps are automated via GitHub Actions:

1. Push tag triggers `release.yml` workflow
2. Builds run in parallel on 3 platforms
3. Artifacts automatically uploaded to GitHub Release
4. `latest` tag updated (if not pre-release)

Only manual steps:
- Version bump
- Changelog update
- Announcements
- Monitoring
