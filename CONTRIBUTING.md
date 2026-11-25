# Contributing to tabsmith

Bug reports, fixes and new features are welcome. This guide covers how to report a problem, build and test
tabsmith, and get a pull request merged.

Found a security problem? Don't open an issue: follow [SECURITY.md](SECURITY.md) instead.

## Reporting a bug

Open an [issue](https://github.com/PedroLaRosa/tabsmith/issues) with:

- `tabsmith -V`, your OS and architecture, your shell and its version, and how you installed tabsmith (one-liner,
  Homebrew, from source);
- what you typed, what you expected and what happened;
- the output of `tabsmith doctor`.

`tabsmith debug logs` shows where the logs are and the end of `pty.log`. Read logs before you paste them: they can
include paths and commands from your session.

**Wrong or missing completions for a specific CLI?** Those come from the
[`@tabsmith/autocomplete-specs`](https://www.npmjs.com/package/@tabsmith/autocomplete-specs) package, which this
repository only bundles. Say which CLI and version so the fix lands in the right place.

## Development setup

You need:

- **Rust 1.85 or later** (CI builds with the latest stable);
- **[just](https://github.com/casey/just)** (optional), which wraps the commands below;
- for the shell integration tests: **zsh**, **fish** and **bash ≥ 4.4**. On macOS, `/bin/bash` is 3.2, so install
  a newer one with `brew install bash` and put it first on your `PATH`. On Windows, put Git Bash first on your `PATH`
  (`C:\Program Files\Git\bin`).

```sh
git clone https://github.com/PedroLaRosa/tabsmith.git && cd tabsmith
just build          # cargo build --locked
just test           # cargo test --locked
```

| Command                        | What it does                                                               |
| ------------------------------ | -------------------------------------------------------------------------- |
| `just build`                   | Debug build                                                                |
| `just test`                    | Unit and integration tests                                                 |
| `just specs-test`              | Load every bundled spec module (release build, as CI does)                 |
| `just install`                 | Install your build into `~/.cargo/bin`                                     |
| `just update-specs [version]`  | Re-bundle `@tabsmith/autocomplete-specs` (default: `assets/SPECS_VERSION`) |
| `just refresh-specs [version]` | Re-bundle the specs, check they load, then install the rebuilt binary      |

Builds use `--locked`: if you change a dependency, commit the updated `Cargo.lock`.

## What CI checks

Every push and pull request runs on Linux, macOS and Windows:

1. `cargo build --locked`
2. `cargo test --locked`, including the architecture test described below
3. `cargo test --locked --release -- --ignored all_bundle_modules` (`just specs-test`)
4. the installers: `shellcheck` on `scripts/install.sh` (Linux), and a parse of `scripts/install.ps1` under Windows
   PowerShell 5.1 and PowerShell 7 (Windows)

Run `just test` and `just specs-test` before you push; a pull request is merged once CI is green.

## How the code is organised

tabsmith follows a hexagonal architecture, sliced by business module:

```text
src/
├── bootstrap/            composition root: CLI, wiring
└── modules/<module>/     completion, dropdown, ghost_text, settings, shell_integration, …
    ├── domain/           entities and value objects, no I/O
    ├── application/      use cases
    └── infrastructure/   adapters: CLI, files, PTY, network, QuickJS
```

[`src/architecture_tests.rs`](src/architecture_tests.rs) enforces the layer rules: a domain or application file
that uses infrastructure, the composition root or an unapproved external crate fails `cargo test`.

Before you change behaviour, read:

- [`docs/SPEC.md`](docs/SPEC.md), the authoritative implementation spec. Its design decisions are numbered (`D15`,
  `D47`, …) and the code refers to them.
- [`docs/ubiquitous-language.md`](docs/ubiquitous-language.md), the names of every domain entity and value object.
  Code, tests, commit messages and docs use exactly these names.

## How we write code

The project's working agreements live in [`.claude/skills/`](.claude/skills/) and apply to humans and AI agents
alike (see [`AGENTS.md`](AGENTS.md)):

| Topic        | Agreement                                                       | Details                                                                      |
| ------------ | --------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Workflow     | Test-driven: red → green → commit → refactor → commit           | [xp-tdd](.claude/skills/xp-tdd/SKILL.md)                                     |
| Tests        | FIRST and Arrange-Act-Assert; no mocks, use in-memory fakes     | [testing-standard](.claude/skills/testing-standard/SKILL.md)                 |
| Code         | Naming, small functions, encapsulation, Law of Demeter          | [coding-standards](.claude/skills/coding-standards/SKILL.md)                 |
| Architecture | Domain → application → infrastructure, one slice per module     | [hexagonal-architecture](.claude/skills/hexagonal-architecture/SKILL.md)     |

## Commits and pull requests

`main` is protected: every change goes through a pull request with green CI.

- **Branch** from `main` as `<type>/<short-description>`, for example `fix/package-manager-prerequisites` or
  `docs/move-releasing-guide`.
- **Commit** with [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`, in the
  imperative and lower case. The types in use are `feat`, `fix`, `refactor`, `test`, `docs`, `build`, `ci`, `chore`
  and `style`. For example:

  ```text
  fix(self_update): time out stalled downloads, not slow ones
  build(specs): bundle @tabsmith/autocomplete-specs@1.0.21
  ```

- **Keep commits small**: each one should build and pass its tests.

## Releases

Releases are cut by maintainers; see [docs/releasing.md](docs/releasing.md).

## Licence

tabsmith is dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your option.
