## Releasing

Maintainers only; `main` is protected, so a release goes through a pull request.

1. Open a PR that bumps `version` in `Cargo.toml` and commits the `Cargo.lock` that follows (any `cargo build`
   updates it; CI and the release build with `--locked`); merge it once CI is green.
2. When it works on your machine, run the release workflow on `main`: Actions → release → Run workflow, or
   `gh workflow run release.yml --ref main`. It tags the commit it built as `vX.Y.Z`.
3. The release workflow tests and builds the six binaries, then waits: approve its `publish` job (the `release`
   environment). It writes `VERSION` and `SHA256SUMS`, signs `SHA256SUMS`, checks the signature against the key the
   binaries carry, and publishes the release. The `verify` jobs then run the one-liners and `tabsmith update` from the
   previous release on macOS, Linux x64/arm64, Alpine and Windows x64/arm64.
   Once they pass, `homebrew` commits the new version and checksums to the tap's `Formula/tabsmith.rb` (from
   `packaging/homebrew/tabsmith.rb`), and `verify-brew` installs it from the tap on macOS and Linux x64/arm64 and runs
   `brew test` and `brew audit --strict --online`. `chocolatey` packs the release's Windows binaries and pushes them to
   the Chocolatey community repository, where each version waits for moderation; `chocolatey-pack` has already
   installed, upgraded and uninstalled the same package on Windows x64 and arm64 (dry runs run it too).
4. If a `verify` job fails, `gh release delete vX.Y.Z` puts `latest` back on the previous release; fix forward with the
   next patch version (an immutable release's tag cannot be used again). The package jobs only run once every `verify`
   job passed. If `verify-brew` fails, the release is fine but the tap serves a bad formula: fix it in the tap, or
   `git revert` its `feat(formula): update tabsmith to X.Y.Z` commit there.

One-time setup, before the first release (needs a repo admin):

1. Generate the release signing key outside the repo with OpenSSL 3 (on macOS `/usr/bin/openssl` is LibreSSL: `brew
install openssl@3` and use `$(brew --prefix openssl@3)/bin/openssl`), and commit the public half:

   ```sh
   openssl genpkey -algorithm ed25519 -out ~/tabsmith-release-key.pem
   openssl pkey -in ~/tabsmith-release-key.pem -pubout -outform DER | tail -c 32 | xxd -p -c 32 > scripts/release-signing-key.pub
   ```

   Never commit the `.pem`. Until this is done, `scripts/release-signing-key.pub` holds a placeholder whose private
   half was thrown away: nothing can be signed for it, so `publish` refuses to release.

2. Create the `release` environment (Settings → Environments) with the maintainer as required reviewer and deployments
   limited to the `main` branch, then `gh secret set RELEASE_SIGNING_KEY --env release < ~/tabsmith-release-key.pem`.
   Move the `.pem` into a password manager and delete the file: a lost key means installed copies cannot update (users
   rerun the installer).
3. Turn on immutable releases (Settings → General → Releases); it only applies to releases created afterwards.
4. Create the `packages` environment, deployments limited to `main` and no required reviewer (the release was already
   approved), for the jobs that update the package managers. Add a read-write deploy key to the tap
   (`PedroLaRosa/homebrew-tap`, Settings → Deploy keys) and store its private half there:
   `gh secret set HOMEBREW_TAP_DEPLOY_KEY --env packages < <key file>`.
5. Create a [Chocolatey community](https://community.chocolatey.org/) account and API key and store the key in the same
   environment: `gh secret set CHOCO_API_KEY --env packages`. A person reviews every version until the package is
   trusted, so push the first one by hand from Windows and get it approved before releasing with the job:

   ```powershell
   gh release download v1.0.0 -R PedroLaRosa/tabsmith --pattern 'tabsmith-win32-*.exe' --dir bin
   .\packaging\chocolatey\pack.ps1 -Version 1.0.0 -BinDir bin
   choco push tabsmith.1.0.0.nupkg --source https://push.chocolatey.org/ --api-key <key>
   ```
