# Security Policy

## Supported Versions

This project is in early alpha (`v0.0.1-alpha`). Until a `v1.0.0` stable
release exists (see [ROADMAP.md](ROADMAP.md), Phase 20), only the latest
tagged release is supported — there is no backport policy yet.

| Version        | Supported |
|-----------------|-----------|
| v0.0.x-alpha    | ✅ (latest only) |

## Reporting a Vulnerability

Because this is an operating system project, "vulnerability" covers more
than the usual web-app categories — it includes things like:
- Memory-safety issues in kernel code that a hostile userland program
  could exploit for privilege escalation.
- Bootloader or firmware-interaction bugs that could allow a malicious
  disk image to execute code before any OS protections exist.
- Flaws in the eventual security model (Phase 15: capability-based
  permissions, signed packages) once that code exists.

**Do not open a public GitHub issue for a security report.** Instead,
report privately through GitHub's private vulnerability reporting feature
on this repository (Security tab → "Report a vulnerability"), which
notifies maintainers without disclosing details publicly.

Please include:
- The phase/component affected (e.g. "Phase 2 bootloader," "Phase 3
  memory manager").
- Steps to reproduce, ideally against a specific commit or tagged
  release.
- The potential impact as you understand it.

## Disclosure Process

1. Report received and acknowledged.
2. Impact assessed and a fix developed, following the same
   one-phase-at-a-time process as normal development — a critical fix
   may justify reopening a "frozen" ADR per the exception process in
   CONTRIBUTING.md.
3. Fix released, with credit to the reporter (unless anonymity is
   requested) in CHANGELOG.md.
