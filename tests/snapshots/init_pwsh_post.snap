# tabsmith post (pwsh): OSC 697 markers from wrapped prompt and PSConsoleHostReadLine functions (SPEC §5.4.5).
if ($env:TABSMITH_SESSION_ID) {
  function Global:__TsOsc([string]$p) { "$([char]27)]697;$p$([char]7)" }
  # Wrap `prompt` unless it is ours already; again whenever it is redefined (`. $PROFILE`, `oh-my-posh init pwsh |
  # iex`, `function prompt {…}`). $true when it wrapped. Our functions run with StrictMode off, as PSReadLine's do.
  function Global:__TsWrap {
    Set-StrictMode -Off
    if ("$function:prompt" -eq "$Global:__TsPrompt") { return $false }
    $Global:__TsUserPrompt = $function:prompt
    function Global:prompt {
      $ok = $global:?; Set-StrictMode -Off; $code = $global:LASTEXITCODE
      # $LASTEXITCODE counts only for a native failure: no new error record, or the one native commands add with
      # $PSNativeCommandUseErrorActionPreference. Otherwise it is stale and a cmdlet failed: 1.
      $native = [object]::ReferenceEquals($Error[0], $Global:__TsErr0) -or $Error[0].FullyQualifiedErrorId -eq 'ProgramExitedWithNonZeroCode'
      $exit = if ($ok) { 0 } elseif ($code -and $native) { $code } else { 1 }
      $sid = $env:TABSMITH_SESSION_ID
      $o = (__TsOsc "OSCUnlock=$sid") + (__TsOsc "ExitCode=$exit") + (__TsOsc 'Shell=pwsh') +
           (__TsOsc "PID=$PID") + (__TsOsc "User=$([Environment]::UserName)")
      if ($PWD.Provider.Name -eq 'FileSystem') { $o += __TsOsc "Dir=$($PWD.ProviderPath)" }
      $rl = if (Get-Module PSReadLine) { Get-PSReadLineOption }
      # PSReadLine draws its ListView prediction list below the prompt: tabsmith keeps out of the way.
      $lv = [int]($rl -and $rl.PredictionViewStyle -eq 'ListView' -and $rl.PredictionSource -ne 'None')
      if (-not $ok) { Write-Error '' -ErrorAction Ignore }   # the user's prompt sees the command's $? (no $Error entry)
      $user = & $Global:__TsUserPrompt
      # After NewCmd, which resets the flag.
      $o += (__TsOsc 'StartPrompt') + $user + (__TsOsc 'EndPrompt') + (__TsOsc "NewCmd=$sid") + (__TsOsc "PwshListView=$lv")
      $global:LASTEXITCODE = $code
      return $o
    }
    $Global:__TsPrompt = $function:prompt
    $true
  }
  $null = __TsWrap
  # PreExec once per accepted line (duplicates and history-filtered lines included): wrap PSReadLine's ReadLine.
  # Ctrl-C and blank lines run nothing and get no PreExec, as in the other shells.
  if (-not (Test-Path Variable:\__TsLoaded) -and (Test-Path Function:\PSConsoleHostReadLine)) {
    $Global:__TsLoaded = $true
    $Global:__TsReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
      $ok = $?   # first statement: PSReadLine's own wrapper reads the last command's status the same way
      Set-StrictMode -Off
      $sid = $env:TABSMITH_SESSION_ID
      # A foreign prompt was just drawn without markers: re-wrap it and track this line from here.
      if (__TsWrap) { [Console]::Write((__TsOsc "OSCUnlock=$sid") + (__TsOsc "ExitCode=$([int](-not $ok))") + (__TsOsc "NewCmd=$sid")) }
      if (-not $ok) { Write-Error '' -ErrorAction Ignore }
      $line = & $Global:__TsReadLine
      if (-not [string]::IsNullOrWhiteSpace($line)) {
        # A line PSReadLine keeps out of its history file (sensitive, or the user's handler) stays out of ours.
        # ponytail: the handler runs a second time here; a handler with side effects sees each line twice.
        $h = if (Get-Module PSReadLine) { (Get-PSReadLineOption).AddToHistoryHandler }
        $nh = try { if ($h -and "$($h.Invoke($line))" -in 'False', 'SkipAdding', 'MemoryOnly') { __TsOsc 'NoHistory' } } catch {}   # never lose the line
        $Global:__TsErr0 = $Error[0]
        [Console]::Write((__TsOsc "OSCLock=$sid") + $nh + (__TsOsc 'PreExec'))
      }
      $line
    }
  }
}
