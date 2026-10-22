# tabsmith pre (pwsh): run the PTY wrapper as a child, then end this shell with its exit code (SPEC §5.4.5).
# Launch only when every argument is an interactive switch (pwsh's shortest unambiguous prefixes): a script path,
# -Command, -File, -EncodedCommand, -CommandWithArgs or -NonInteractive must run in this shell, untouched.
$__tsIs = { param([string[]]$names) [bool]($names | Where-Object {   # 'name:shortest prefix'
  $n, $min = $_ -split ':'; $__tsA.Length -ge $min.Length -and $n.StartsWith($__tsA, 'OrdinalIgnoreCase') }) }
$__tsInteractive = $true
$__tsArgs = @([Environment]::GetCommandLineArgs() | Select-Object -Skip 1)
for ($__tsI = 0; $__tsInteractive -and $__tsI -lt $__tsArgs.Count; $__tsI++) {
  $__tsA = $__tsArgs[$__tsI] -replace '^(--?|/)', ''
  if (& $__tsIs 'workingdirectory:wo', 'wd:wd', 'executionpolicy:ex', 'ep:ep', 'windowstyle:w') { $__tsI++ }
  elseif (-not (& $__tsIs 'nologo:nol', 'noexit:noe', 'noprofile:nop', 'noprofileloadtime:noprofileloadtime',
                          'login:l', 'interactive:i', 'sta:sta', 'mta:mta')) { $__tsInteractive = $false }
}
if ($TABSMITH_SHOULD_LAUNCH -eq 0 -and $__tsInteractive -and [Environment]::UserInteractive -and
    -not [Console]::IsInputRedirected -and -not [Console]::IsOutputRedirected) {
  $env:TABSMITH_SHELL = (Get-Process -Id $PID).Path
  $global:LASTEXITCODE = $null
  # The inner pwsh gets this shell's switches (-ExecutionPolicy, -wd, ...) and no second banner. A leading `-`
  # makes `_ pty` treat them as shell arguments. ponytail: AllHosts profiles (profile.ps1) already ran in this
  # shell and run again in the inner one; the pre block would have to live in $PROFILE.CurrentUserAllHosts.
  & {{BIN}} _ pty --shell $env:TABSMITH_SHELL -- -NoLogo @__tsArgs
  # The outer shell ends with the wrapped one; if the wrapper could not start at all, this shell carries on.
  if ($null -ne $global:LASTEXITCODE) { [Environment]::Exit($global:LASTEXITCODE) }
}
