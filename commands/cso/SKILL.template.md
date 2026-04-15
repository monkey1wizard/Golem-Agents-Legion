---
name: cso
description: "Chief Security Officer audit. Scans the codebase for OWASP Top 10 and STRIDE threats, then writes findings to the active plan. Run before any /ship that touches auth, data handling, or public API surface."
---

# /cso

Automated security audit. No interactive questions — scan the codebase and write findings.

## Role

Chief Security Officer. Your job is to find exploitable vulnerabilities with concrete evidence, not theoretical risks.

This is the post-implementation security audit for the workflow review stage. It complements, but does not replace, the planning-stage engineering review lane.

## When to Use

- After implementation review on branches that touch authentication, data storage, input handling, or public API surface
- Before any `/ship` that touches authentication, data storage, input handling, or public API surface
- As a standalone security review on any new feature branch
- When `/gal status` reports Security Review: MISSING
- When `/gal whats-next` recommends `/cso` for security-sensitive scope

## Confidence Gate

Only report findings with **8/10 or higher confidence**. Verify each finding independently before including it. Below 8/10, do not include — noise is worse than silence.

## Step 1 — Identify Active Plan

Read `.dev/state.md` to find the active plan file. If no active plan exists, note that findings will be reported but not written to a plan file.

## Step 2 — Scan: OWASP Top 10

Check each category. For each potential finding, confirm it is exploitable in this codebase before including.

| OWASP Category | What to Look For |
| --- | --- |
| A01 Broken Access Control | Direct object references, missing authz checks, CORS misconfiguration, path traversal |
| A02 Cryptographic Failures | Secrets in code, weak hashing (MD5/SHA1), unencrypted sensitive data in storage or transit |
| A03 Injection | SQL injection, command injection, LDAP injection, XSS, template injection, deserialization |
| A04 Insecure Design | Missing threat model, security not in requirements, unsafe design patterns that cannot be patched |
| A05 Security Misconfiguration | Default credentials, verbose error messages revealing internals, unnecessary features enabled |
| A06 Vulnerable Components | Outdated dependencies with known CVEs, unmaintained libraries in the dependency tree |
| A07 Auth & Session Failures | Weak session token generation, missing expiry, no rate limiting on auth endpoints |
| A08 Software Integrity Failures | Unsigned packages, auto-update without integrity check, build pipeline compromise vectors |
| A09 Logging Failures | Absence of security event audit log, credentials or PII appearing in logs |
| A10 SSRF | User-controlled URLs fetched server-side without allowlist or validation |

## Step 3 — Scan: STRIDE Threat Model

| Threat | Check |
| --- | --- |
| Spoofing | Can an attacker impersonate another user or service? |
| Tampering | Can data be modified in transit or at rest without detection? |
| Repudiation | Can an attacker deny performing an action? |
| Information Disclosure | Is sensitive data exposed to unauthorized parties? |
| Denial of Service | Can an attacker exhaust resources or block legitimate access? |
| Elevation of Privilege | Can an attacker gain higher privileges than intended? |

## False-Positive Exclusions

Do NOT report these as issues:

- Self-signed certificates in local dev environments
- `console.log` statements in development-only code paths
- TODO comments about future security improvements
- Unused / unreachable code
- Dependencies only used in test tooling
- Security headers absent in non-web backends (e.g., gRPC, CLI tools)
- Rate limiting absent on genuinely internal-only endpoints

## Step 4 — Format Each Finding

For each confirmed finding (≥ 8/10 confidence):

```
**[SEVERITY] FINDING-NNN: <title>**
- File: `<path>:<line>`
- Exploit scenario: <one concrete sentence — what an attacker does and what they gain>
- Recommended fix: <specific code change or configuration>
- Confidence: <N>/10
```

Severity levels: CRITICAL / HIGH / MEDIUM / LOW

## Step 5 — Write Back to Plan

In the active plan file, append under `## Review Results`:

```markdown
### Security Review

**Date:** <today>
**Findings:** <N total> — <X critical>, <Y high>, <Z medium>, <W low>

#### Summary

<One paragraph: scope of the review, what was scanned, overall security posture.>

#### Open Findings

<Paste each CRITICAL and HIGH finding here. MEDIUM and LOW in a collapsible section.>

#### Remediation Tracking

| Finding | Severity | Status |
| --- | --- | --- |
| FINDING-001 | CRITICAL | OPEN |

<!-- SECURITY_REVIEW: FINDINGS-OPEN -->
```

If no findings at HIGH or above: write `<!-- SECURITY_REVIEW: CLEAR -->` instead.

Tell the user the finding count by severity and which (if any) must be resolved before `/ship`.
