$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\tools\install-core.ps1')
if (Test-WebView2Runtime -ReadVersion { param($path) $null }) { throw 'WebView2 missing runtime was accepted' }
if (Test-WebView2Runtime -ReadVersion { param($path) '0.0.0.0' }) { throw 'WebView2 zero version was accepted' }
if (Test-WebView2Runtime -ReadVersion { param($path) 'not-a-version' }) { throw 'WebView2 malformed version was accepted' }
if (-not (Test-WebView2Runtime -ReadVersion { param($path) if ($path -like 'HKLM:*') { '153.0.4234.48' } })) { throw 'WebView2 machine installation was missed' }
if (-not (Test-WebView2Runtime -ReadVersion { param($path) if ($path -like 'HKCU:*') { '153.0.4234.48' } })) { throw 'WebView2 user installation was missed' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('screen-time-install-test-' + [guid]::NewGuid().ToString('N'))
$source = Join-Path $root 'package'
$installed = Join-Path $root 'installed'
New-Item -ItemType Directory -Path $source,$installed -Force | Out-Null
try {
    $names = @('screen-time.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt','uninstall-user.ps1','uninstall-core.ps1','docs\ARQUITECTURA.md','docs\VALIDACION.md','docs\DEPENDENCIAS.md','docs\PUBLICAR.md')
    New-Item -ItemType Directory -Path (Join-Path $source 'docs'),(Join-Path $installed 'docs') -Force | Out-Null
    foreach ($name in $names) { [IO.File]::WriteAllText((Join-Path $source $name),'new'); [IO.File]::WriteAllText((Join-Path $installed $name),'old') }
    $failed = $false
    try { Install-AppFiles $source $installed { throw 'Injected integration failure' } } catch { if ($_.Exception.Message -ne 'Injected integration failure') { throw }; $failed = $true }
    if (-not $failed) { throw 'Expected injected failure' }
    foreach ($name in $names) { if ([IO.File]::ReadAllText((Join-Path $installed $name)) -ne 'old') { throw "Rollback failed: $name" } }
    Install-AppFiles $source $installed
    foreach ($name in $names) { if ([IO.File]::ReadAllText((Join-Path $installed $name)) -ne 'new') { throw "Update failed: $name" } }
    $fresh = Join-Path $root 'fresh'
    try { Install-AppFiles $source $fresh { throw 'Initial install failure' } } catch { if ($_.Exception.Message -ne 'Initial install failure') { throw } }
    if (@(Get-ChildItem -LiteralPath $fresh -Recurse -File).Count) { throw 'Initial install rollback left files behind' }
    Install-AppFiles $source $fresh
    if ([IO.File]::ReadAllText((Join-Path $fresh 'screen-time.exe')) -ne 'new') { throw 'Initial install failed' }
    Remove-Item -LiteralPath (Join-Path $source 'LICENSE')
    $failed = $false
    try { Install-AppFiles $source $installed } catch { $failed = $true }
    if (-not $failed -or [IO.File]::ReadAllText((Join-Path $installed 'README.md')) -ne 'new') { throw 'Incomplete package was not rejected safely' }
    $data = Join-Path $root 'data'
    for ($i=1; $i -le 7; $i++) {
        $folder = Join-Path $data ('backups\pre-install-20260928-00000' + $i)
        New-Item -ItemType Directory -Path $folder -Force | Out-Null
        [IO.File]::WriteAllText((Join-Path $folder 'app_usage.csv'),'history')
    }
    $restore = Join-Path $data 'backups\pre-install-20261015-090000'
    New-Item -ItemType Directory -Path $restore -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $restore 'app_usage.csv'),'legacy data')
    [IO.File]::WriteAllText((Join-Path $restore 'app_usage.settings.json'),'old settings')
    [IO.File]::WriteAllText((Join-Path $data 'app_usage.csv'),'new UTC data')
    [IO.File]::WriteAllText((Join-Path $data 'app_usage.daily.json'),'new index')
    Restore-AppDataBackup -DataDirectory $data -BackupDirectory $restore
    if ([IO.File]::ReadAllText((Join-Path $data 'app_usage.csv')) -ne 'legacy data' -or (Test-Path -LiteralPath (Join-Path $data 'app_usage.daily.json'))) { throw 'Data rollback failed' }
    $failedCopies = @(Get-ChildItem -LiteralPath $restore -Directory -Filter 'failed-update-*')
    if ($failedCopies.Count -ne 1 -or [IO.File]::ReadAllText((Join-Path $failedCopies[0].FullName 'app_usage.csv')) -ne 'new UTC data') { throw 'Failed update data was not preserved' }
    $badBackup = $false
    try { Restore-AppDataBackup -DataDirectory $data -BackupDirectory $root } catch { $badBackup = $true }
    if (-not $badBackup) { throw 'Unsafe backup path was accepted' }
    $unrelated = Join-Path $data 'backups\my-backup'
    New-Item -ItemType Directory -Path $unrelated -Force | Out-Null
    Remove-OldAppBackups $data $installed
    if (@(Get-ChildItem (Join-Path $data 'backups') -Directory -Filter 'pre-install-*').Count -ne 5 -or -not (Test-Path -LiteralPath $unrelated)) { throw 'Backup retention failed' }
    Write-Output 'Installer: WebView2 presence/absence, rollback, update, incomplete package, backup retention checks passed.'
} finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ([IO.Path]::GetDirectoryName($resolved).TrimEnd('\') -eq [IO.Path]::GetTempPath().TrimEnd('\') -and [IO.Path]::GetFileName($resolved).StartsWith('screen-time-install-test-')) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
