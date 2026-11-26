# Security policy

## Reporting a vulnerability

**Please don't report security problems in public issues, pull requests or discussions.**

Report them privately with GitHub's
[private vulnerability reporting](https://github.com/PedroLaRosa/tabsmith/security/advisories/new). The report
stays visible only to you and the maintainers until a fix is published.

Include what you can:

- the tabsmith version (`tabsmith -V`), your OS and architecture, your shell, and how you installed tabsmith;
- the steps to reproduce, or a proof of concept;
- what an attacker could do with it, and what they need first (local access, a malicious repository, a hostile
  network, …).

## Supported versions

Only the [latest release](https://github.com/PedroLaRosa/tabsmith/releases/latest) receives security fixes. A fix
ships as a new release; older releases are not patched.

Installs made with the one-liners update themselves: tabsmith checks for a new release at most once a day, when a
terminal tab opens. Homebrew and source installs have to be updated by hand; see
[Update](README.md#update) in the README.

## What's in scope

tabsmith runs inside your terminal session, so these areas deserve the most scrutiny:

- **Installers and self-update** ([`scripts/`](scripts/), the `self_update` module): download, checksum and signature
  checks, and replacing the binary in place.
- **The PTY wrapper and shell integration**: tabsmith sees what you type, and `tabsmith setup` writes code that your
  shell sources at startup.
- **Completion specs**: they run in an embedded QuickJS engine and can run commands to build dynamic suggestions
  (git branches, npm scripts, docker containers, …).
- **LLM ghost text** (opt-in): what leaves your machine, the filtering of secret-looking history lines, and the
  handling of API keys, which are read from environment variables only.

## How releases are protected

- Every release's `SHA256SUMS` is signed with an ed25519 release key. Its public half is
  [`scripts/release-signing-key.pub`](scripts/release-signing-key.pub) and is compiled into the binary.
- Self-update installs a release only if that signature verifies and the downloaded binary matches its checksum.
- The installers check the downloaded binary against `SHA256SUMS` before they install it.
- Releases are immutable and attested by GitHub. You can check one with the GitHub CLI:

  ```sh
  gh release verify v1.0.1 --repo PedroLaRosa/tabsmith
  ```

See [docs/releasing.md](docs/releasing.md) for how a release is built, signed and verified.
