#!/bin/bash
# phase10-greps.sh: the Phase 10 acceptance greps, one line per check:  OK / FAIL / REVIEW  <label>
#   OK      the grep is empty, as required
#   FAIL    it must be empty and is not (the hits follow, indented)
#   REVIEW  it lists the places a human must read (the hits follow); the expectation is in the label
# Run from the repo root at the end of every step that moves code, and at the close. At phase10-start (b0fe04d)
# the A checks and E1 FAIL on purpose: doctor.rs and cli.rs are still alive and diagnostics is not registered there.
#   usage: scripts/hexagonal-refactor/phase10-greps.sh [base-rev]   (base-rev default phase10-start, else b0fe04d)
cd "$(git rev-parse --show-toplevel)" || exit 2
BASE="${1:-phase10-start}"
git rev-parse -q --verify "$BASE^{commit}" >/dev/null || BASE=b0fe04d
DG=src/diagnostics
# "Production" lines of a path: tests.rs files are dropped, and the test-only items of a file are cut: a column-0
# `#[cfg(test)]` followed by `mod ..` ends the production part (the test module is last by convention); followed by
# any other item it skips that item (a `;` line, or up to the closing `}` at column 0). Indented test-only items
# (inside an impl) are still read as production, which can only add false positives.
prod() { # prod <grep -E pattern> <paths...>
  local pat="$1"; shift
  git ls-files -co --exclude-standard -- "$@" 2>/dev/null | grep '\.rs$' | grep -v '/tests\.rs$' | while read -r f; do
    [ -e "$f" ] || continue
    awk -v F="$f" '
      skipall { next }
      initem { if ($0 ~ /^}/) initem = 0; next }
      pending {
        if ($0 ~ /^#\[/) next
        pending = 0
        if ($0 ~ /^(pub\(crate\) )?mod /) { skipall = 1; next }
        if ($0 ~ /;[ \t]*$/ && $0 !~ /\{/) next
        initem = 1; next
      }
      /^#\[cfg\(test\)\]/ { pending = 1; next }
      { print F ":" FNR ":" $0 }' "$f"
  done | grep -E "$pat" | grep -vE '^[^:]+:[0-9]+:\s*//'
}
fail=0
expect_empty() { # expect_empty <label> <output>
  if [ -z "$2" ]; then echo "OK      $1"; else echo "FAIL    $1"; echo "$2" | sed 's/^/          /' | cut -c1-200 | head -12; fail=1; fi
}
review() { echo "REVIEW  $1"; [ -n "$2" ] && echo "$2" | sed 's/^/          /' | cut -c1-200 | head -${3:-12}; }
comments_out() { grep -vE '^[^:]+:[0-9]+:\s*//'; }

echo "== A. the legacy files and names are gone"
expect_empty "A1 src/doctor.rs and src/cli.rs deleted" "$(ls src/doctor.rs src/cli.rs 2>/dev/null)"
expect_empty "A2 main.rs declares neither mod doctor nor mod cli" "$(grep -nE '^\s*mod (doctor|cli);' src/main.rs)"
grep -qE '^mod diagnostics;' src/main.rs && echo "OK      A3 main.rs declares mod diagnostics" || { echo "FAIL    A3 main.rs does not declare mod diagnostics"; fail=1; }
expect_empty "A4 no crate::doctor / crate::cli / doctor:: / bare cli:: path anywhere (infrastructure::cli:: and bootstrap::cli:: are not one)" \
  "$(grep -rnE 'crate::(doctor|cli)\b|crate::\{[^}]*\b(doctor|cli)\b|(^|[^_a-zA-Z:])(doctor|cli)::[a-zA-Z]' src --include=*.rs | comments_out)"
expect_empty "A5 no comment or string names doctor.rs or src/cli.rs as a file that exists (settings_catalog.rs doc lines, the example in architecture_tests.rs; bootstrap/cli.rs is legitimate)" \
  "$(grep -rnE '\bdoctor\.rs\b|src/cli\.rs' src --include=*.rs)"
expect_empty "A6 settings::infrastructure::load_or_defaults is deleted: no file, no mod line, no caller (lenient_live_settings.rs names it in a LOCKED test name and its doc line; allowed)" \
  "$(grep -rn 'load_or_defaults' src --include=*.rs | grep -v 'src/terminal_session/infrastructure/adapters/lenient_live_settings.rs')"
expect_empty "A7 nothing outside types.rs calls types::now_secs / now_ms (D7: types.rs itself stays for Phase 11; architecture_tests.rs fixtures name crate::types as a string)" \
  "$(grep -rnE 'crate::types\b|\btypes::now_(ms|secs)\b' src --include=*.rs | grep -v '^src/architecture_tests.rs' | comments_out)"
expect_empty "A8 the free fns diagnostic/diagnostics/to_toml/debug_logs/windows_build/restore_cursor_on_ctrl_c exist only in their new homes (no leftover copy in bootstrap)" \
  "$(grep -rnE '\bfn (diagnostic|diagnostics|debug_logs)\b' src --include=*.rs | grep -v '^src/diagnostics/')"

echo "== B. layers and dependencies (production code of $DG)"
expect_empty "B1 no legacy flat module (infrastructure included: rules 1-2 do not read it)" \
  "$(prod 'crate::(types|engine|parser|suggest|pty|menu|render|icons|doctor|cli|tracker|keys|shell_parser|js|spec|inline|integrations)\b' $DG)"
expect_empty "B2 no bootstrap, terminal_session, dropdown, completion, history, specs (design edges: diagnostics -> shared, settings, shell_integration, ghost_text; the plan also allows specs and terminal_session but the bridges make them unnecessary: relax this line and say why if the design differs)" \
  "$(prod 'crate::(bootstrap|terminal_session|dropdown|completion|history|specs)\b' $DG)"
expect_empty "B3 no infrastructure of another module (rule 3) and no other module's application layer except ports/DTOs: shell_integration, settings and ghost_text are read through domain only" \
  "$(prod 'crate::(settings|ghost_text|shell_integration)::(infrastructure|application)' $DG)"
expect_empty "B4 no use case of another module (a *_use_case file or *UseCase type outside diagnostics)" \
  "$(prod 'crate::(settings|history|completion|ghost_text|dropdown|specs|shell_integration|terminal_session)::[a-z_:A-Z]*(_use_case|UseCase)' $DG)"
expect_empty "B5 IntegrationInstaller (an application service rule 3 cannot see, P14) is named only by bootstrap" \
  "$(prod 'IntegrationInstaller|integration_installer|integration_failures_error|embedded_spec_bundle|build_specs_module|JsonFileSettingsRepository|StateRestartReminderRepository|FileDotfileRepository|OsShellEnvironment' $DG)"
expect_empty "B6 no re-export (pub use / pub(crate) use) in diagnostics, bootstrap" \
  "$(grep -rnE '^\s*pub(\(crate\))? use ' $DG src/bootstrap 2>/dev/null)"
expect_empty "B7 nothing wider than pub(crate) in diagnostics, bootstrap" \
  "$(grep -rnE '^\s*pub (fn|struct|enum|trait|const|static|type|mod|use)\b' $DG src/bootstrap 2>/dev/null)"
review "B8 domain fields: private; only a *Parts struct may be pub(crate)" \
  "$(prod '^[^:]+:[0-9]+:\s+pub(\(crate\))? [a-z_]+:' $DG/domain)"
expect_empty "B9 anyhow and clap only in infrastructure/cli (no anyhow in the module builder, the adapters, domain, application)" \
  "$(prod '\b(anyhow|clap)\b' $DG/domain $DG/application $DG/infrastructure/adapters $DG/infrastructure/diagnostics_module.rs)"
expect_empty "B10 libc, signal_hook, windows_sys, crossterm, std::os::windows, std::os::unix only in infrastructure/adapters (never domain/application)" \
  "$(prod '\b(libc|signal_hook|windows_sys|crossterm)\b|std::os::(windows|unix)' $DG/domain $DG/application)"
expect_empty "B11 serde, serde_json, json! only in infrastructure" "$(prod '\b(serde|serde_json)\b|json!' $DG/domain $DG/application)"
expect_empty "B12 print, println, eprint, eprintln, dbg, logger, paint/epaint/sgr (tty styling) only in infrastructure" \
  "$(prod '\b(e?print(ln)?|dbg)!|\blogger\b|\b(e?paint|sgr|bold|stdout_tty)\(' $DG/domain $DG/application)"
expect_empty "B13 no I/O, env, process, clock, thread or channel in domain/application (std fs/env/process/thread/net/io, IsTerminal, Command, SystemTime, Instant::now, current_dir/current_exe, consts::OS/ARCH, mpsc)" \
  "$(prod 'std::(fs|env|process|thread|net|io)\b|IsTerminal|\bCommand::new|SystemTime|UNIX_EPOCH|Instant::now|current_dir|current_exe|consts::(OS|ARCH)|mpsc|var_os|env::var' $DG/domain $DG/application)"
review "B14 compile-time env!() in domain/application (the TABSMITH_TERM == CARGO_PKG_VERSION rule): prefer the version as a fact so a test can fix it" \
  "$(prod '\benv!\(' $DG/domain $DG/application)"
expect_empty "B15 no cfg in domain/application (the design keeps every platform branch in infrastructure/adapters; the Windows rules read facts, None off Windows). Relax with a reason if the planner gates a port method instead" \
  "$(prod '#\[cfg(_attr)?\((windows|unix|not|target_os|all|any|feature)|cfg!\(' $DG/domain $DG/application)"
review "B16 threads and channels live in infrastructure/adapters only (the spinner and the Ctrl-C watcher)" \
  "$(prod 'thread::|mpsc::|Arc<|AtomicBool' $DG | grep -v '/infrastructure/adapters/')"
review "B17 Rc<dyn ..> holders: use cases, the check evaluator and the module (single thread)" \
  "$(prod 'Rc<dyn ' $DG | awk -F: '{print $1}' | sort | uniq -c)" 30
review "B18 more than one pub(crate) type per file (allowed: a trait with its #[cfg(test)] double, a private helper enum, *Parts)" \
  "$(git ls-files -co --exclude-standard $DG | grep '\.rs$' | grep -v '/mod\.rs$\|/tests\.rs$' | while read -r f; do n=$(awk '/^#\[cfg\(test\)\]/ {skip=1} !skip && /^pub\(crate\) (struct|enum|trait)/ {c++} END{print c+0}' "$f"); [ "$n" -gt 1 ] && echo "$f: $n"; done)"
review "B19 bodyless #[cfg(test)] item whose line holds an unpaired ( or [ (the fitness scanner then reads the next production lines as test code; use \\x5b)" \
  "$(git ls-files -co --exclude-standard $DG | grep '\.rs$' | while read -r f; do awk -v F="$f" '/^[ \t]*#\[cfg\(test\)\]/ {getline n; o=gsub(/[\[(]/,"&",n); c=gsub(/[\])]/,"&",n); if (n !~ /\{/ && o!=c) print F": "n}' "$f"; done)"
review "B20 files outside mod.rs and the three layer folders (rule 5 checks this too)" \
  "$(git ls-files -co --exclude-standard $DG | grep -vE '^src/diagnostics/(mod\.rs|(domain|application|infrastructure)/)')"

echo "== C. cfg and platform code (the gate cannot compile windows on this machine unless the mingw target is installed)"
cfg_lines() { grep -hE 'cfg\((windows|unix|not\(|target_os|all\()|cfg!\(|cfg_attr' | sed -E 's/^\s+//' | sort; }
OLD_CFG="$(git ls-tree -r --name-only "$BASE" -- src/doctor.rs src/cli.rs src/bootstrap $DG | grep '\.rs$' | while read -r f; do git show "$BASE:$f"; done | cfg_lines)"
NEW_CFG="$( (git ls-files -co --exclude-standard src/doctor.rs src/cli.rs src/bootstrap $DG | grep '\.rs$' | while read -r f; do [ -e "$f" ] && cat "$f"; done) | cfg_lines)"
REVIEW_CFG="$(comm -3 <(echo "$OLD_CFG") <(echo "$NEW_CFG") | sed -E 's/^\t/  ADDED   /; t; s/^/  REMOVED /')"
review "C1 cfg lines ($(echo "$OLD_CFG" | grep -c .) at $BASE, $(echo "$NEW_CFG" | grep -c .) now) of doctor.rs, cli.rs, bootstrap and diagnostics: a move shows nothing; the expected REMOVED/ADDED pairs are #[cfg_attr(not(windows), allow(unused_mut))] (the windows checks become facts) and the #[cfg(windows)] arm of os_outcome; every other line needs a reason" "$REVIEW_CFG" 40
review "C2 every cfg(windows) item of diagnostics (read each by hand: use paths, windows_sys names, handle types, unsafe blocks; nothing here is compiled by a Linux host build)" \
  "$(grep -rnE 'cfg\(windows\)|std::os::windows|windows_sys|cfg!\(windows' $DG src/bootstrap 2>/dev/null)" 30
review "C3 every cfg(unix) item of diagnostics (signal_hook + libc::SIGINT live in the cursor-restore adapter only)" \
  "$(grep -rnE 'cfg\(unix\)|signal_hook|libc::' $DG 2>/dev/null)" 20
if rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$' && command -v x86_64-w64-mingw32-gcc >/dev/null; then
  echo "          windows target installed: the gate runs cargo check --locked --all-targets --target x86_64-pc-windows-gnu"
else
  echo "FAIL    C4 the windows target or the mingw gcc is missing on this machine: the gate prints NO windows line, so cfg(windows) code is unchecked (rustup target add x86_64-pc-windows-gnu; apt-get install gcc-mingw-w64-x86-64)"; fail=1
fi

echo "== D. tests and the lock"
for t in doctor_group_soft_fix_abort doctor_script_bins_skip_orphans doctor_session_check_skips_pwsh doctor_version_parsing diagnostic_toml; do
  n=$(grep -rnE "fn $t\b" src --include=*.rs | wc -l | tr -d ' ')
  [ "$n" = 1 ] && echo "OK      D1 fn $t defined once" || { echo "FAIL    D1 fn $t defined $n times (a split keeps the name on exactly one part)"; fail=1; }
done
for t in should_give_the_settings_the_repository_loads should_word_the_warning_as_settings_then_the_error_and_each_cause should_fall_back_to_the_default_settings_when_they_cannot_be_loaded; do
  n=$(grep -rnE "fn $t\b" src --include=*.rs | wc -l | tr -d ' ')
  [ "$n" = 1 ] && echo "OK      D2 fn $t defined once (re-homed with load_or_defaults' deletion)" || { echo "FAIL    D2 fn $t defined $n times (testlock MISSING against $BASE if 0)"; fail=1; }
done
review "D3 exactly one #[ignore] in the tree (quickjs_spec_runtime/tests.rs, all_bundle_modules)" "$(grep -rnE '^\s*#\[ignore' src --include=*.rs)"
review "D4 testlock: python3 -B scripts/hexagonal-refactor/testlock.py . ef807c1 and . $BASE ; MISSING 0 and DUPLICATE 0 against both; CHANGED against ef807c1 grows by at most the diagnostics tests listed in the step briefs" \
  "$(python3 -B scripts/hexagonal-refactor/testlock.py . "$BASE" | grep -E '^(MISSING|DUPLICATE|CHANGED) ' | cut -c1-160)"
review "D5 a diagnostics test that spawns a process, reads the real env or the real home (must use TempDir, a fake probe or an env-free path)" \
  "$(git ls-files -co --exclude-standard $DG | grep '\.rs$' | xargs grep -nE 'Command::new|std::env::(set_var|var)|HOME' 2>/dev/null | grep -v '/adapters/os_')"

echo "== E. the fitness test"
echo "          cargo test --locked architecture   (diagnostics is in REGISTERED_MODULES)"
grep -q '"diagnostics"' src/architecture_tests.rs && echo "OK      E1 diagnostics registered" || { echo "FAIL    E1 diagnostics not registered"; fail=1; }
expect_empty "E2 no src/diagnostics.rs next to the module folder" "$(ls src/diagnostics.rs 2>/dev/null)"
review "E3 LEGACY_MODULES still lists the dead names cli and doctor (precedent: pty and tracker stayed, O13; Phase 11 empties the list). A comment in architecture_tests.rs may no longer name src/cli.rs" \
  "$(sed -n '/^const LEGACY_MODULES/,/^];/p' src/architecture_tests.rs | tr -d ' \n' | cut -c1-200)"

echo "== F. names that live only inside diagnostics (outside it only bootstrap wires them)"
expect_empty "F1 doctor domain and application names only inside diagnostics, bootstrap and architecture_tests" \
  "$(grep -rnE '\b(DoctorRun|DoctorCheckCatalog|CheckSeverity|CheckOutcome|CheckGroup|ToolVersion|UsernameScrubber|RunDoctorUseCase|DoctorReporter|TerminalDoctorReporter|Spinner|DiagnosticReport|DiagnosticSection)\b' src --include=*.rs | comments_out | grep -vE '^src/(diagnostics|bootstrap|architecture_tests)')"
expect_empty "F2 DiagnosticsModule is built in exactly one place (bootstrap/cli.rs fn diagnostics_module) and used by the three commands" \
  "$(grep -rn 'build_diagnostics_module(' src --include=*.rs | grep -vE '^src/(diagnostics|bootstrap/cli\.rs)')"
expect_empty "F3 SetConsoleCtrlHandler and the SIGINT watcher (Signals::new) are named by exactly one file of diagnostics (the cursor-restore adapter) and by neither doctor.rs nor cli.rs" \
  "$( n=$(grep -rlE 'SetConsoleCtrlHandler|Signals::new' $DG 2>/dev/null | wc -l | tr -d ' '); [ "$n" = 1 ] || echo "files in $DG naming it: $n"; grep -lE 'SetConsoleCtrlHandler|Signals::new' src/doctor.rs src/cli.rs 2>/dev/null )"
expect_empty "F4 println!/eprintln! in diagnostics only in infrastructure/cli and infrastructure/adapters/terminal_doctor_reporter.rs" \
  "$(prod '\b(e?print(ln)?)!' $DG | grep -vE '/infrastructure/(cli/|adapters/(terminal_doctor_reporter|spinner|cursor_restorer)\.rs)')"
expect_empty "F5 the 'Failed to fix: ' prefix, the hint line and the root message exist once each in production code" \
  "$( for s in 'Failed to fix: ' 'tabsmith still not working\? Check ~/\.tabsmith/logs/pty\.log' 'Running doctor as root is not supported\.'; do n=$(prod "$s" src | wc -l | tr -d ' '); [ "$n" = 1 ] || echo "$s : $n"; done)"

echo "== G. every user-visible literal of doctor.rs and cli.rs (at $BASE) survives somewhere in diagnostics or bootstrap"
G_MISSING="$(python3 -I "$(dirname "$0")/phase10-literals.py" "$BASE")"
review "G1 literals of the old files not found verbatim in the new tree (a string split across lines with a backslash or turned into a format! piece shows here: read each; clap help texts (doc comments) are not literals and are covered by the golden capture)" "$G_MISSING" 40

echo "== Z. the module's file list"
review "Z git ls-files src/diagnostics | sort equals the file list of the design's module map (read by eye at the close)" \
  "$(git ls-files -co --exclude-standard $DG | wc -l | tr -d ' ') files"
exit $fail
