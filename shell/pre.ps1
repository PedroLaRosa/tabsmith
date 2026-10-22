# tabsmith pre (pwsh): run the PTY wrapper as a child, then end this shell with it (SPEC §5.4.5).
$__tsArgs = [Environment]::GetCommandLineArgs()
$__tsCmd = ($__tsArgs | Where-Object { $_ -imatch '^-(c|command|f|file)$' }).Count -gt 0
$__tsNoExit = ($__tsArgs | Where-Object { $_ -imatch '^-noe(xit)?$' }).Count -gt 0
if ($TABSMITH_SHOULD_LAUNCH -eq 0 -and [Environment]::UserInteractive -and
    -not [Console]::IsInputRedirected -and -not [Console]::IsOutputRedirected -and
    (-not $__tsCmd -or $__tsNoExit)) {
  $env:TABSMITH_SHELL = (Get-Process -Id $PID).Path
  & {{BIN}} _ pty --shell $env:TABSMITH_SHELL
  Stop-Process -Id $PID          # the outer shell ends with the wrapped one
}
