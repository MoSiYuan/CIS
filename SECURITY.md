# Security Policy

## Reporting Security Vulnerabilities

If you discover a security vulnerability in CIS, please report it responsibly.

### How to Report

**Email**: security@cis-project.org
**GitHub**: Use [GitHub's private vulnerability reporting](https://github.com/CIS-Project/CIS/security/advisories)

Please include:
- Description of the vulnerability
- Steps to reproduce (if applicable)
- Potential impact assessment
- Suggested fix (if known)

### What to Expect

- **Response**: Within 48 hours (weekends excluded)
- **Acknowledgement**: We'll confirm receipt of your report
- **Updates**: Regular status updates on the fix progress
- **Coordinated Disclosure**: We'll work with you on disclosure timing

## Supported Versions

Currently supported versions with security updates:

| Version | Supported Until | Status |
|---------|-----------------|--------|
| v1.1.x | Until v1.2.0 release | ✅ Active |
| v1.0.x | Until v1.1.0 release | ⚠️ Maintenance |

**Note**: Older versions may not receive security updates. Please upgrade to the latest version.

## Security Features

### Implemented ✅

- **DID Authentication**: Challenge-response with nonce replay protection
  - P1-6: WebSocket nonce cache
  - Unique nonce per authentication
  - 5-minute TTL with automatic cleanup

- **Memory Encryption**: Private/Public domain separation
  - P0-3: KDF security warnings documented
  - Planned Argon2id upgrade (Phase 2)

- **Access Control**: Network ACL (whitelist/blacklist)
  - DID-based access control
  - Quarantine mode for restricted peers

- **WASM Sandbox**: 4-layer path validation
  - P1-12: Security boundaries defined
  - FD limits, file size limits, symlink depth limits

- **Key Management**: Secure file permissions
  - P0-2: Cross-platform permission handling
  - 0o600 (Unix) and ACLs (Windows)
  - Permission verification after setting

- **Memory Safety**: RAII and bounds checking
  - Rust language guarantees
  - Minimal `unsafe` blocks
  - P0-6: Batch memory limits (100MB cap)

### Planned ⏳

- **Argon2id KDF**: Upgrade from SHA256
- **E2EE**: End-to-end encryption for federation
- **Audit Logging**: Security event logging
- **Rate Limiting**: Connection rate limits
- **Penetration Testing**: Professional audit

## Security Audits

### Completed Audits

#### 2026-02: GLM & Kimi AI Comprehensive Review

**Scope**: Full codebase security and architecture review

**Findings**:
- Critical issues: 7
- High-priority: 14
- Medium-priority: 15

**Status**:
- ✅ All critical issues fixed (P0: 100%)
- ✅ High-priority: 50% complete (P1: 7/14)
- 📊 Report: [COMPLETION_REPORT.md](docs/plan/v1.1.7/COMPLETION_REPORT.md)

**Key Fixes**:
- WebSocket replay attack protection
- DAG parallel execution (3-5x performance)
- Memory limits and OOM prevention
- Key permission hardening
- Dependency version unification

### Upcoming Audits

- **Q2 2026**: External professional security audit
- **Q3 2026**: Penetration testing by third-party firm

## Vulnerability Response Process

### Severity Classification

| Severity | Response Time | Example |
|----------|--------------|---------|
| **Critical** | 48 hours | RCE, SQL injection, auth bypass |
| **High** | 7 days | DoS, info disclosure, privilege escalation |
| **Medium** | 30 days | XSS, CSRF, misconfigurations |
| **Low** | 90 days | Minor issues, documentation |

### Response Steps

1. **Triage**: Assess severity and impact within 24 hours
2. **Confirmation**: Acknowledge receipt and severity classification
3. **Development**: Create fix in private branch
4. **Testing**: Verify fix doesn't break functionality
5. **Release**: Publish security update
6. **Disclosure**: Publish advisory after 30 days (or immediately if public exploit)

### Security Updates

Security updates will be released as patch version increments:

- `v1.1.5` → `v1.1.6`: Security fixes only
- Changelog will mark security updates with `[SECURITY]`
- Upgrade instructions included in release notes

## Security Best Practices

### For Users

1. **Keep Updated**
   ```bash
   cis update
   cis upgrade --security-only
   ```

2. **Secure Keys**
   - Never share private DID keys
   - Use strong passphrases
   - Enable key encryption if available

3. **Network Security**
   - Use WSS (WebSocket Secure) for remote connections
   - Enable ACL for network access
   - Review allowed peers regularly

4. **Memory Management**
   - Use private domain for sensitive data
   - Regular cleanup of old memory
   - Monitor memory usage

### For Developers

1. **Input Validation**
   ```rust
   // Always validate user input
   let input = input.trim();
   if input.len() > MAX_LENGTH {
       return Err(CisError::Validation("Input too long"));
   }
   ```

2. **Dependencies**
   ```bash
   # Audit dependencies
   cargo audit
   cargo supply chains update
   ```

3. **Secrets Management**
   ```rust
   // Never hardcode secrets
   // ❌ Bad
   let api_key = "sk-1234567890";

   // ✅ Good
   let api_key = std::env::var("API_KEY")?;
   ```

4. **Code Review**
   - All changes require review
   - Security-sensitive changes need 2 reviewers
   - Use `cargo clippy` and `cargo audit` in CI

5. **Testing**
   ```rust
   // Include security tests
   #[test]
   fn test_replay_attack_prevented() {
       // Test nonce reuse detection
       let nonce = "test-nonce";
       cache.verify_and_use(nonce).unwrap();
       assert!(cache.verify_and_use(nonce).is_err());
   }
   ```

## Security Configuration

### Default Security Settings

```toml
# ~/.cis/config.toml
[security]
# Enable automatic security updates
auto_update = true

# Require encryption for private memory
require_encryption = true

# Network security
[network]
# Enable ACL by default
acl_enabled = true
# Allowed DIDs (whitelist)
allowed_dids = ["did:cis:*"]
# Blocked DIDs (blacklist)
blocked_dids = []

# WASM sandbox limits
[wasm]
max_fd = 32
max_file_size = "100MB"
max_symlink_depth = 8
```

### Hardening Recommendations

For production deployments:

1. **Enable all security features**
   ```bash
   cis config --security.encryption required
   cis config --network.acl enabled
   ```

2. **Use reverse proxy** (for Matrix API)
   - Nginx/Apache with SSL/TLS
   - Rate limiting
   - IP whitelisting

3. **Regular updates**
   ```bash
   # Enable automatic security updates
   cis config --auto-update true
   ```

4. **Monitor logs**
   ```bash
   cis logs --security --tail
   ```

## Security Contacts

- **Security Team**: security@cis-project.org
- **Lead Maintainer: [GitHub username]
- **GitHub Security**: https://github.com/CIS-Project/CIS/security/advisories

## Disclosure Policy

### Coordinated Disclosure

We follow responsible disclosure practices:

1. **Report**: Researcher reports vulnerability privately
2. **Acknowledge**: We confirm receipt within 48 hours
3. **Fix**: We develop fix within severity SLA
4. **Test**: We verify fix doesn't break functionality
5. **Release**: We publish security update
6. **Disclose**: Researcher publishes details (with credit)

### Credit

Security researchers will be credited in:
- Release changelog
- Security advisory
- Hall of Fame (if desired)

### Embargo

Default embargo period is **30 days** after fix release, but can be extended if:
- Exploit is public knowledge
- Active attacks in the wild
- User risk is critical

## Related Resources

- [Security Audit Report](docs/plan/v1.1.7/claude/CIS_COMPREHENSIVE_REVIEW_REPORT.md)
- [Issue Tracker](docs/plan/v1.1.7/claude/CONSOLIDATED_ISSUES_LIST.md)
- [Completion Report](docs/plan/v1.1.7/COMPLETION_REPORT.md)

---

**Last Updated**: 2026-02-18
**Policy Version**: 2.0
**Next Review**: 2026-05-18
