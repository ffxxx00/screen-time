$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\tools\uninstall-core.ps1')
$root = Join-Path ([IO.Path]::GetTempPath()) ('screen-time-uninstall-test-' + [guid]::NewGuid().ToString('N'))
$install = Join-Path $root 'Programs\Screen Time'
$data = Join-Path $root 'data'
$links = Join-Path $root 'Desktop'
$registryRoot = 'HKCU:\Software\ScreenTimeUninstallTest_' + [guid]::NewGuid().ToString('N')
$runKey = Join-Path $registryRoot 'Run'
$uninstallKey = Join-Path $registryRoot 'Uninstall\Screen Time'
try {
    New-Item -ItemType Directory -Path $install,$data,$links,(Join-Path $install 'docs') -Force | Out-Null
    foreach ($name in @('screen-time.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt','uninstall-user.ps1','uninstall-core.ps1')) {
        [IO.File]::WriteAllText((Join-Path $install $name),'installed')
    }
    foreach ($name in @('ARQUITECTURA.md','VALIDACION.md','DEPENDENCIAS.md','PUBLICAR.md')) {
        [IO.File]::WriteAllText((Join-Path (Join-Path $install 'docs') $name),'installed')
    }
    [IO.File]::WriteAllText((Join-Path $install 'personal.txt'),'keep')
    $webViewCache = Join-Path $install 'screen-time.exe.WebView2'
    New-Item -ItemType Directory -Path $webViewCache | Out-Null
    [IO.File]::WriteAllText((Join-Path $webViewCache 'cache.bin'),'cache')
    [IO.File]::WriteAllText((Join-Path $data 'app_usage.csv'),'history')
    $exe = Join-Path $install 'screen-time.exe'
    $shell = New-Object -ComObject WScript.Shell
    $appLink = Join-Path $links 'Screen Time.lnk'
    $shortcut = $shell.CreateShortcut($appLink); $shortcut.TargetPath = $exe; $shortcut.Save()
    $otherLink = Join-Path $links 'Otra app.lnk'
    $shortcut = $shell.CreateShortcut($otherLink); $shortcut.TargetPath = 'C:\Windows\notepad.exe'; $shortcut.Save()
    New-Item -Path $runKey,$uninstallKey -Force | Out-Null
    New-ItemProperty -Path $runKey -Name 'Screen Time' -Value ('"' + $exe + '"') -PropertyType String | Out-Null
    New-ItemProperty -Path $uninstallKey -Name DisplayName -Value 'Screen Time' -PropertyType String | Out-Null
    New-ItemProperty -Path $uninstallKey -Name InstallLocation -Value $install -PropertyType String | Out-Null

    $rejected = $false
    try { Uninstall-AppFiles $data @($links) $runKey $uninstallKey { param($path) } }
    catch { $rejected = $true }
    if (-not $rejected -or [IO.File]::ReadAllText((Join-Path $data 'app_usage.csv')) -ne 'history') { throw 'Unexpected directory was not rejected safely' }

    $failed = $false
    try {
        Uninstall-AppFiles $install @($links) $runKey $uninstallKey { param($path) throw 'Injected shutdown failure' }
    } catch { if ($_.Exception.Message -ne 'Injected shutdown failure') { throw }; $failed = $true }
    if (-not $failed -or -not (Test-Path -LiteralPath $exe) -or -not (Test-Path -LiteralPath $appLink)) { throw 'Shutdown failure modified installation' }

    $script:stopped = $false
    Uninstall-AppFiles $install @($links) $runKey $uninstallKey { param($path) if ($path -ne $exe) { throw 'Wrong process target' }; $script:stopped = $true }
    if (-not $script:stopped -or (Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath $appLink) -or (Test-Path -LiteralPath $webViewCache) -or -not (Test-Path -LiteralPath $otherLink)) { throw 'App files or shortcuts were not removed selectively' }
    if ((Get-ItemProperty -Path $runKey -Name 'Screen Time' -ErrorAction SilentlyContinue) -or (Test-Path $uninstallKey)) { throw 'Registry entries were not removed' }
    if (-not (Test-Path -LiteralPath (Join-Path $install 'personal.txt')) -or [IO.File]::ReadAllText((Join-Path $data 'app_usage.csv')) -ne 'history') { throw 'Uninstall deleted user data' }
    foreach ($name in @('uninstall-user.ps1','uninstall-core.ps1')) {
        Copy-Item -LiteralPath (Join-Path (Join-Path $PSScriptRoot '..\tools') $name) -Destination (Join-Path $install $name)
    }
    $entryOutput = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $install 'uninstall-user.ps1') -InstallDirectory $install -DataDirectory $data -ShortcutDirectories $links -RunKeyPath $runKey -UninstallKeyPath $uninstallKey 2>&1
    if ($LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath (Join-Path $install 'uninstall-user.ps1')) -or (Test-Path -LiteralPath (Join-Path $install 'uninstall-core.ps1'))) { throw "Uninstall entrypoint failed: $entryOutput" }
    if ([IO.File]::ReadAllText((Join-Path $data 'app_usage.csv')) -ne 'history') { throw 'Entrypoint deleted history' }
    Write-Output 'Uninstaller: graceful shutdown, selective removal, registry and history retention passed.'
} finally {
    if (Test-Path $registryRoot) { Remove-Item -Path $registryRoot -Recurse -Force }
    $resolved = [IO.Path]::GetFullPath($root)
    if ([IO.Path]::GetDirectoryName($resolved).TrimEnd('\') -eq [IO.Path]::GetTempPath().TrimEnd('\') -and [IO.Path]::GetFileName($resolved).StartsWith('screen-time-uninstall-test-')) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
