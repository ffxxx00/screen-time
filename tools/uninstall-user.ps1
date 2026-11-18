param(
    [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA 'Programs\Screen Time'),
    [string]$DataDirectory = (Join-Path $env:LOCALAPPDATA 'Screen Time'),
    [string[]]$ShortcutDirectories = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs')),
    [string]$RunKeyPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
    [string]$UninstallKeyPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Screen Time'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'uninstall-core.ps1')
Uninstall-AppFiles -InstallDirectory $InstallDirectory -ShortcutDirectories $ShortcutDirectories `
    -RunKeyPath $RunKeyPath -UninstallKeyPath $UninstallKeyPath
Write-Output "Screen Time desinstalado. Tu historial permanece en: $DataDirectory"
