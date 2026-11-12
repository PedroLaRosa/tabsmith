@echo off
rem Windows PowerShell 5.1 may still default to TLS 1.0, which GitHub refuses: TLS 1.2 (3072) is on before the download.
powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor 3072; & ([scriptblock]::Create((irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1))) %*"
