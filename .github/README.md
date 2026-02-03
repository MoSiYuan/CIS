# GitHub Configuration

This directory contains GitHub-specific configuration files.

## Workflows

### CI (`workflows/ci.yml`)
Runs on every push and PR:
- Code formatting check (`cargo fmt`)
- Clippy linting
- Unit tests
- Cross-platform build test

### Release (`workflows/release.yml`)
Triggered by pushing a version tag (`v*`):
- Builds for macOS (Intel + Apple Silicon)
- Builds for Linux (x86_64)
- Builds for Windows (x86_64)
- Creates installers:
  - macOS: `.dmg`, `.app.tar.gz`
  - Linux: `.AppImage`, `.deb`, `.tar.gz`
  - Windows: `.msi`, `.zip`
- Creates GitHub Release with all artifacts
- Updates `latest` tag

### Nightly (`workflows/nightly.yml`)
Runs daily at 00:00 UTC:
- Builds latest `main` branch
- Runs benchmarks
- Creates/updates `nightly` pre-release
- Keeps artifacts for 7 days

## Issue Templates

- **Bug Report**: For reporting bugs
- **Feature Request**: For suggesting new features

## Pull Request Template

Standard PR template with:
- Description
- Type of change
- Testing checklist
- General checklist

## Setting Up Repository Secrets

For code signing, add these secrets in GitHub Settings:

### macOS Code Signing
- `MACOS_CERTIFICATE`: Base64-encoded `.p12` certificate
- `MACOS_CERTIFICATE_PWD`: Certificate password

### Windows Code Signing
- `WINDOWS_CERTIFICATE`: Base64-encoded `.pfx` certificate  
- `WINDOWS_CERTIFICATE_PWD`: Certificate password

### Generating Base64 Certificate

```bash
# macOS
cat certificate.p12 | base64 | pbcopy

# Linux
cat certificate.p12 | base64 -w 0

# Windows (PowerShell)
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certificate.p12"))
```

## Creating a Release

1. Update version in `Cargo.toml` files
2. Update `CHANGELOG.md`
3. Commit and push
4. Create and push tag:
   ```bash
   git tag -a v1.0.0 -m "Release v1.0.0"
   git push origin v1.0.0
   ```
5. GitHub Actions automatically builds and creates release

## Release Artifacts

Each release includes:

| Platform | Artifacts |
|----------|-----------|
| macOS | `CIS-{version}-macos.dmg`<br>`CIS-{version}-macos.app.tar.gz` |
| Linux | `CIS-{version}-x86_64.AppImage`<br>`cis_{version}_amd64.deb`<br>`cis-{version}-linux-x86_64.tar.gz` |
| Windows | `CIS-{version}-x86_64.msi`<br>`CIS-{version}-windows-x86_64.zip` |
