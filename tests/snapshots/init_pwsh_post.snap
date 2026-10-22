# tabsmith post (pwsh): OSC 697 markers from a wrapped prompt function + PSReadLine history hook (SPEC §5.4.5).
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
  if (Get-Module PSReadLine) {   # PreExec: PSReadLine calls this when a line is accepted (inferred; verify ordering)
    $Global:__TsPrevHist = (Get-PSReadLineOption).AddToHistoryHandler
    Set-PSReadLineOption -AddToHistoryHandler {
      param([string]$line)
      [Console]::Write((__TsOsc "OSCLock=$env:TABSMITH_SESSION_ID") + (__TsOsc 'PreExec'))
      if ($Global:__TsPrevHist) { return & $Global:__TsPrevHist $line } else { return $true }
    }
  }
}
