# Security Policy

## Supported Versions

KubeCove is in beta. Security fixes are prepared for the latest published beta
release line and `main`.

| Version | Supported |
| ------- | --------- |
| Latest published beta | Yes |
| Older beta releases | No |
| Local source builds | No; update to a fixed release or `main` |

## Reporting a Vulnerability

Report vulnerabilities through GitHub private vulnerability reporting:

https://github.com/Timpan4/kubecove/security/advisories/new

If private reporting is unavailable, open a public issue that asks for a private
contact path. Do not include exploit details, credentials, kubeconfig contents,
tokens, certificates, or cluster-specific sensitive data in a public issue.

Useful reports include:

- affected KubeCove version or commit
- operating system
- a short impact summary
- minimal reproduction steps
- whether real cluster credentials, kubeconfig data, or Kubernetes resources are
  exposed or modified

Security-sensitive areas include kubeconfig handling, token or certificate
exposure, frontend access to shell or filesystem capabilities, Kubernetes
operation guardrails, updater signing, and installer or local privilege issues.

Expected handling:

- The report is triaged privately.
- Accepted vulnerabilities get a fix on `main` and, when practical, a beta patch
  release.
- Declined reports get a brief reason, such as intended local-only behavior,
  required Kubernetes RBAC, unsupported old beta, or no demonstrated security
  impact.
- Public details should wait until a fix or mitigation is available.
