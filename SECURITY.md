# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Chibby, please report it responsibly.

**Do not open a public issue.** Instead, email security concerns to: **security@okapian.com**

Please include:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and aim to provide a fix or mitigation plan within 7 days.

## Scope

Chibby runs locally on your machine. Security concerns include:

- Pipeline command injection (malicious step definitions)
- Secret/environment variable exposure
- Unauthorized file system access
- Supply chain risks in dependencies
