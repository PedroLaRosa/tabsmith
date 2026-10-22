# tabsmith post (pwsh): OSC 697 markers from wrapped prompt and PSConsoleHostReadLine functions (SPEC §5.4.5).
if ($env:TABSMITH_SESSION_ID -and -not $Global:__TsLoaded) {
  $Global:__TsLoaded = $true
  $Global:__TsUserPrompt = $function:prompt
  function Global:__TsOsc([string]$p) { "$([char]27)]697;$p$([char]7)" }
  function Global:prompt {
    $ok = $global:?; $code = $global:LASTEXITCODE
    $exit = if ($ok) { 0 } elseif ($code) { $code } else { 1 }
    $sid = $env:TABSMITH_SESSION_ID
    $o = (__TsOsc "OSCUnlock=$sid") + (__TsOsc "ExitCode=$exit") + (__TsOsc 'Shell=pwsh') +
         (__TsOsc "PID=$PID") + (__TsOsc "User=$([Environment]::UserName)")
    if ($PWD.Provider.Name -eq 'FileSystem') { $o += __TsOsc "Dir=$($PWD.ProviderPath)" }
    $o += (__TsOsc 'StartPrompt') + (& $Global:__TsUserPrompt) + (__TsOsc 'EndPrompt') + (__TsOsc "NewCmd=$sid")
    $global:LASTEXITCODE = $code
    return $o
  }
  # PreExec once per accepted line (duplicates and history-filtered lines included): wrap PSReadLine's ReadLine.
  # Ctrl-C and blank lines run nothing and get no PreExec, as in the other shells.
  if (Test-Path Function:\PSConsoleHostReadLine) {
    $Global:__TsReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
      $line = & $Global:__TsReadLine
      if (-not [string]::IsNullOrWhiteSpace($line)) {
        [Console]::Write((__TsOsc "OSCLock=$env:TABSMITH_SESSION_ID") + (__TsOsc 'PreExec'))
      }
      $line
    }
  }
}
