#!/bin/bash
# phase9-greps.sh: the Phase 9 acceptance greps (the "9.8 greps"), one line per check:  OK / FAIL / REVIEW  <label>
#   OK      the grep is empty, as required
#   FAIL    it must be empty and is not (the hits follow, indented)
#   REVIEW  it lists the places a human must read (the hits follow); the expectation is in the label
# Run from the repo root at the end of every step that moves code, and at the close. At phase9-start (858b7b5)
# the A and some B checks FAIL on purpose: pty.rs and tracker.rs are still alive there.
#   usage: scripts/hexagonal-refactor/phase9-greps.sh [base-rev]      (base-rev default phase9-start, for the cfg check)
cd "$(git rev-parse --show-toplevel)" || exit 2
BASE="${1:-phase9-start}"
TS=src/terminal_session
# Lines of $TS that are not inside a trailing `#[cfg(test)] mod tests {..}` / `mod tests;` / a tests.rs file are
# "production" for these greps: strip tests.rs files and cut every file at its top-level `#[cfg(test)]` marker.
prod() { # prod <grep -E pattern> <paths...>: matching lines of production code only (approximation, like the fitness scan)
  local pat="$1"; shift
  git ls-files -co --exclude-standard -- "$@" | grep '\.rs$' | grep -v '/tests\.rs$' | while read -r f; do
    [ -e "$f" ] || continue
    awk -v F="$f" '/^#\[cfg\(test\)\]/ {skip=1} !skip {print F ":" FNR ":" $0}' "$f"
  done | grep -E "$pat" | grep -vE '^[^:]+:[0-9]+:\s*//'
}
fail=0
expect_empty() { # expect_empty <label> <output>
  if [ -z "$2" ]; then echo "OK      $1"; else echo "FAIL    $1"; echo "$2" | sed 's/^/          /' | cut -c1-200 | head -12; fail=1; fi
}
review() { echo "REVIEW  $1"; [ -n "$2" ] && echo "$2" | sed 's/^/          /' | cut -c1-200 | head -${3:-12}; }

echo "== A. the legacy files and names are gone"
expect_empty "A1 src/pty.rs and src/tracker.rs deleted" "$(ls src/pty.rs src/tracker.rs 2>/dev/null)"
expect_empty "A2 main.rs declares neither mod pty nor mod tracker" "$(grep -nE '^\s*mod (pty|tracker);' src/main.rs)"
expect_empty "A3 no crate::pty / crate::tracker / pty:: / tracker:: path anywhere (portable_pty:: is not one)" \
  "$(grep -rnE 'crate::(pty|tracker)\b|(^|[^_a-z:])(pty|tracker)::' src --include=*.rs | grep -vE '^[^:]+:[0-9]+:\s*//')"
expect_empty "A4 no comment names pty.rs or tracker.rs as a file that exists (settings_catalog.rs doc lines, the example in architecture_tests.rs)" \
  "$(grep -rnE '\b(pty|tracker)\.rs\b' src --include=*.rs)"
expect_empty "A5 types::Msg and types::Model are gone (headless_model::Model is legitimate)" \
  "$(grep -rnE 'types::(Msg|Model)\b|\bMsg::|:\s*Msg\b' src --include=*.rs)"
expect_empty "A6 src/types.rs holds neither Msg nor Model nor an import only they used" \
  "$(grep -nE 'enum Msg|type Model|completion_result::CompletionResult' src/types.rs)"

echo "== B. layers and dependencies (production code of $TS)"
expect_empty "B1 no legacy flat module (infrastructure included: rule 1-2 do not read it)" \
  "$(prod 'crate::(types|engine|parser|suggest|pty|menu|render|icons|doctor|cli|tracker|keys|shell_parser|js|spec|inline|integrations)\b' $TS)"
expect_empty "B2 no shell_integration, diagnostics or bootstrap" \
  "$(prod 'crate::(shell_integration|diagnostics|bootstrap)\b' $TS)"
expect_empty "B3 no specs in production (only the test-only terminal_session -> specs edge, deviation 25)" \
  "$(prod 'crate::specs\b' $TS)"
review "B4 another module's application/infrastructure: only DTOs and ports of completion (CompletionRequest), nothing else" \
  "$(prod 'crate::(settings|history|completion|ghost_text|dropdown|shell_integration|diagnostics)::(application|infrastructure)' $TS)"
expect_empty "B5 no use case of another module (a *_use_case file or *UseCase type outside terminal_session)" \
  "$(prod 'crate::(settings|history|completion|ghost_text|dropdown|specs|shell_integration|diagnostics)::[a-z_:]*(_use_case|UseCase)' $TS)"
expect_empty "B6 no re-export (pub use / pub(crate) use) in terminal_session, bootstrap or the legacy cli" \
  "$(grep -rnE '^\s*pub(\(crate\))? use ' $TS src/bootstrap src/cli.rs 2>/dev/null)"
expect_empty "B7 nothing wider than pub(crate)" \
  "$(grep -rnE '^\s*pub (fn|struct|enum|trait|const|static|type|mod|use)\b' $TS src/bootstrap 2>/dev/null)"
review "B8 domain fields: only the *Parts structs may be pub(crate)" \
  "$(prod '^[^:]+:[0-9]+:\s+pub(\(crate\))? [a-z_]+:' $TS/domain)"
expect_empty "B9 anyhow and clap only in infrastructure/cli and infrastructure/pty" \
  "$(prod '\b(anyhow|clap)\b' $TS/domain $TS/application $TS/infrastructure/adapters $TS/infrastructure/terminal_session_module.rs)"
expect_empty "B10 alacritty_terminal, portable_pty, crossterm, libc, signal_hook, windows_sys only in infrastructure/adapters and infrastructure/pty (never domain/application)" \
  "$(prod '\b(alacritty_terminal|portable_pty|crossterm|libc|signal_hook|windows_sys)\b' $TS/domain $TS/application)"
expect_empty "B11 serde, serde_json, json! only in infrastructure" "$(prod '\b(serde|serde_json)\b|json!' $TS/domain $TS/application)"
expect_empty "B12 print, println, eprint, eprintln, dbg, logger only in infrastructure" \
  "$(prod '\b(e?print(ln)?|dbg)!|\blogger\b' $TS/domain $TS/application)"
expect_empty "B13 no I/O, clock, thread or channel in domain/application (std fs/env/process/thread/net/io, SystemTime, Instant::now, current_dir, mpsc outside the CompletionEngine double)" \
  "$(prod 'std::(fs|env|process|thread|net|io)\b|SystemTime::now|UNIX_EPOCH|Instant::now|current_dir|mpsc' $TS/domain $TS/application | grep -v 'ports/completion_engine.rs')"
review "B14 threads and channels live in infrastructure/pty only (plus the Arc<dyn Clock + Send + Sync> of ReportSessionContextUseCase)" \
  "$(prod 'thread::|mpsc::|Arc<' $TS | grep -v '/infrastructure/pty/')"
review "B15 Rc<dyn ..> holders: use cases, services and the session runner (single thread, deviation O5)" \
  "$(prod 'Rc<dyn ' $TS | awk -F: '{print $1}' | sort | uniq -c)" 30
review "B16 more than one pub(crate) type per file (allowed: *Parts, a private helper enum, a trait with its #[cfg(test)] double and Call enum, ShellArg-style parameter types; DeferredMarker and LockSettlement have their own files)" \
  "$(git ls-files -co --exclude-standard $TS | grep '\.rs$' | grep -v '/mod\.rs$\|/tests\.rs$' | while read -r f; do n=$(awk '/^#\[cfg\(test\)\]/ {skip=1} !skip && /^pub\(crate\) (struct|enum|trait)/ {c++} END{print c+0}' "$f"); [ "$n" -gt 1 ] && echo "$f: $n"; done)"

review "B17 bodyless #[cfg(test)] item whose line holds an unpaired ( or [ (a b\"\\x1b[\" literal): the fitness scanner then reads the following production lines as test code (P8-T49; use \\x5b)" \
  "$(git ls-files -co --exclude-standard $TS | grep '\.rs$' | while read -r f; do awk -v F="$f" '/^[ \t]*#\[cfg\(test\)\]/ {getline n; o=gsub(/[\[(]/,"&",n); c=gsub(/[\])]/,"&",n); if (n !~ /\{/ && o!=c) print F": "n}' "$f"; done)"

echo "== C. cfg and platform code"
cfg_lines() { grep -hE 'cfg\((windows|unix|not\(|target_os|all\()|cfg!\(' | sed -E 's/^\s+//' | sort; }
OLD_CFG="$(git ls-tree -r --name-only "$BASE" -- $TS src/bootstrap src/cli.rs src/pty.rs src/tracker.rs | grep '\.rs$' | while read -r f; do git show "$BASE:$f"; done | cfg_lines)"
NEW_CFG="$( (git ls-files -co --exclude-standard $TS src/bootstrap src/cli.rs src/pty.rs src/tracker.rs | grep '\.rs$' | while read -r f; do [ -e "$f" ] && cat "$f"; done) | cfg_lines)"
REVIEW_CFG="$(comm -3 <(echo "$OLD_CFG") <(echo "$NEW_CFG") | sed -E 's/^\t/  ADDED   /; t; s/^/  REMOVED /')"
review "C1 cfg lines ($(echo "$OLD_CFG" | grep -c .) at $BASE, $(echo "$NEW_CFG" | grep -c .) now) of pty.rs, tracker.rs, cli.rs, bootstrap and terminal_session at $BASE vs now: a move shows nothing; REMOVED 'if cfg!(test)' is the TerminalWriter port; every ADDED/REMOVED line needs a reason" "$REVIEW_CFG" 30
echo "          then: cargo check --locked --all-targets --target x86_64-pc-windows-gnu (the gate runs it)"

echo "== D. tests and the panic hook"
review "D1 the one #[cfg(test)] panic injection call: exactly one call site in production code, in the per-key loop" "$(grep -rn 'maybe_panic' src | cut -c1-140)" 10
review "D2 exactly one #[ignore] in the tree (quickjs_spec_runtime/tests.rs, all_bundle_modules); testlock MISSING 0 / DUPLICATE 0 is the other half (testlock.py against ef807c1 and phase9-guards)" \
  "$(grep -rnE '^\s*#\[ignore' src --include=*.rs)"
review "D3 D9 strings: the self re-exec path of debug_shell_survives_closed_terminal names its new module" \
  "$(grep -rn 'tests::debug_shell_survives_closed_terminal' src)"

echo "== E. the fitness test"
echo "          cargo test --locked architecture   (terminal_session is in REGISTERED_MODULES)"
grep -q '"terminal_session"' src/architecture_tests.rs && echo "OK      E1 terminal_session registered" || { echo "FAIL    E1 terminal_session not registered"; fail=1; }

echo "== F. names that live only inside terminal_session"
expect_empty "F1 session services and use cases are named only inside terminal_session (the legacy pty.rs is exempt until 9.6d)" \
  "$(grep -rnE '\b(KeystrokeHandler|CompletionRequester|OverlayPainter|SessionFilePoller|ShellOutputFeeder|SessionUseCases)\b' src | grep -v '^src/terminal_session/' | grep -v '^src/pty.rs:' | grep -v '^src/architecture_tests.rs:')"
expect_empty "F2 no legacy decoder or tracker names (OutFilter, StdinKeyDecoder, Extracted, TAG_PROMPT, Splitter, Piece::)" \
  "$(grep -rnE '\b(OutFilter|StdinKeyDecoder|Extracted|TAG_PROMPT|Splitter)\b|\bPiece::' src)"
expect_empty "F3 write_stdout only in stdout_terminal_writer.rs; std::io::stdout( in terminal_session only in that file, key_tester_command.rs, debug_shell_command.rs and session_launcher.rs" \
  "$(grep -rn 'write_stdout' src | grep -v 'stdout_terminal_writer\.rs:'; grep -rn 'std::io::stdout(' $TS | grep -vE '/(stdout_terminal_writer|key_tester_command|debug_shell_command|session_launcher)\.rs:')"
expect_empty "F4 SessionUseCases::new only in session_launcher.rs and test_fixtures.rs" \
  "$(grep -rn 'SessionUseCases::new' src | grep -vE '/(session_launcher|test_fixtures)\.rs:')"
# a catch_unwind inside an inline `#[cfg(test)] mod tests` (after its attribute line) is test code, so it does not count
f5_production_hits() {
  for file in $(grep -rl catch_unwind $TS | grep -vE '/tests\.rs$|/(session_runner|pty_session)\.rs$'); do
    awk '/^#\[cfg\(test\)\]$/ { cfg = 1; next } cfg && /^mod tests/ { exit } { cfg = 0 } /catch_unwind/ { print FILENAME ":" FNR ": " $0 }' "$file"
  done
}
expect_empty "F5 catch_unwind in terminal_session production only in session_runner.rs and pty_session.rs" \
  "$(f5_production_hits)"
echo "== Z. the module's file list"
review "Z git ls-files src/terminal_session | sort equals the file list of the design's Module map (read by eye at 9.7)" \
  "$(git ls-files -co --exclude-standard $TS | wc -l | tr -d ' ') files"
exit $fail
