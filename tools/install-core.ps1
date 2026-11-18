function Test-WebView2Runtime {
    param([scriptblock]$ReadVersion = {
        param($registryPath)
        (Get-ItemProperty -Path $registryPath -Name pv -ErrorAction SilentlyContinue).pv
    })
    $client = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $machine = if ([Environment]::Is64BitOperatingSystem) {
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$client"
    } else {
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$client"
    }
    $user = "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$client"
    foreach ($path in @($machine, $user)) {
        $raw = try { & $ReadVersion $path } catch { $null }
        $version = $null
        if ([version]::TryParse([string]$raw, [ref]$version) -and $version -gt [version]'0.0.0.0') {
            return $true
        }
    }
    return $false
}

function Install-AppFiles {
    param([Parameter(Mandatory)][string]$SourceDirectory, [Parameter(Mandatory)][string]$InstallDirectory, [scriptblock]$AfterInstall = {})
    $names = @('screen-time.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt','uninstall-user.ps1','uninstall-core.ps1','docs\ARQUITECTURA.md','docs\VALIDACION.md','docs\DEPENDENCIAS.md','docs\PUBLICAR.md')
    foreach ($name in $names) {
        if (-not (Test-Path -LiteralPath (Join-Path $SourceDirectory $name) -PathType Leaf)) { throw "Falta el archivo del paquete: $name" }
    }
    $root = [IO.Path]::GetFullPath($InstallDirectory)
    New-Item -ItemType Directory -Path $root -Force | Out-Null
    $transaction = Join-Path $root ('.update-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $transaction | Out-Null
    $changed = [Collections.Generic.List[string]]::new()
    $existing = @{}
    $rollbackErrors = @()
    try {
        foreach ($name in $names) {
            $source = Join-Path $SourceDirectory $name
            $staged = Join-Path $transaction $name
            New-Item -ItemType Directory -Path (Split-Path $staged) -Force | Out-Null
            Copy-Item -LiteralPath $source -Destination $staged
            if ((Get-FileHash -LiteralPath $source).Hash -ne (Get-FileHash -LiteralPath $staged).Hash) { throw "La copia no coincide: $name" }
            $destination = Join-Path $root $name
            New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
            $existing[$name] = Test-Path -LiteralPath $destination
            if ($existing[$name]) { Copy-Item -LiteralPath $destination -Destination (Join-Path $transaction ($name + '.previous')) }
        }
        foreach ($name in $names) {
            $changed.Add($name)
            $destination = Join-Path $root $name
            if ($existing[$name]) { [IO.File]::Replace((Join-Path $transaction $name), $destination, (Join-Path $transaction ($name + '.replaced'))) }
            else { [IO.File]::Move((Join-Path $transaction $name), $destination) }
        }
        & $AfterInstall
    } catch {
        $originalError = $_
        $rollbackErrors = @()
        foreach ($name in $changed) {
            try {
                $destination = Join-Path $root $name
                if ($existing[$name]) { Copy-Item -LiteralPath (Join-Path $transaction ($name + '.previous')) -Destination $destination -Force }
                elseif (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination }
            } catch { $rollbackErrors += $_.Exception.Message }
        }
        if ($rollbackErrors.Count) { throw "Falló la actualización: $originalError. Copias de recuperación en $transaction. Errores al restaurar: $($rollbackErrors -join '; ')" }
        throw $originalError
    } finally {
        # Remove only this invocation's generated child directory after verifying its parent.
        if (-not $rollbackErrors -and [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($transaction)) -eq $root) {
            Remove-Item -LiteralPath $transaction -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Restore-AppDataBackup {
    param([Parameter(Mandatory)][string]$DataDirectory, [Parameter(Mandatory)][string]$BackupDirectory)
    $root = [IO.Path]::GetFullPath($DataDirectory).TrimEnd('\')
    $backup = [IO.Path]::GetFullPath($BackupDirectory).TrimEnd('\')
    if ([IO.Path]::GetDirectoryName($backup) -ne (Join-Path $root 'backups') -or -not [IO.Path]::GetFileName($backup).StartsWith('pre-install-')) { throw 'Ruta de respaldo no válida.' }
    if (-not (Test-Path -LiteralPath (Join-Path $backup 'app_usage.csv') -PathType Leaf)) { throw 'El respaldo no contiene el historial anterior.' }
    # Keep any writes made by the failed new version before restoring older data.
    $failed = Join-Path $backup ('failed-update-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $failed | Out-Null
    $current = @(Get-ChildItem -LiteralPath $root -File -Filter 'app_usage.*')
    foreach ($file in $current) { Copy-Item -LiteralPath $file.FullName -Destination $failed }
    foreach ($file in $current) { Remove-Item -LiteralPath $file.FullName }
    Get-ChildItem -LiteralPath $backup -File -Filter 'app_usage.*' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $root }
}

function Remove-OldAppBackups {
    param([string]$DataDirectory, [string]$InstallDirectory)
    $backupRoot = [IO.Path]::GetFullPath((Join-Path $DataDirectory 'backups'))
    if (Test-Path -LiteralPath $backupRoot) {
        $old = Get-ChildItem -LiteralPath $backupRoot -Directory | Where-Object { $_.Name -match '^pre-install-\d{8}-\d{6}$' } | Sort-Object Name -Descending | Select-Object -Skip 5
        foreach ($directory in $old) {
            if (($directory.Attributes -band [IO.FileAttributes]::ReparsePoint) -or [IO.Path]::GetDirectoryName($directory.FullName) -ne $backupRoot) { continue }
            $items = @(Get-ChildItem -LiteralPath $directory.FullName -Force)
            if (@($items | Where-Object { $_.PSIsContainer -or $_.Name -notlike 'app_usage.*' -or ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) }).Count) { continue }
            foreach ($item in $items) { Remove-Item -LiteralPath $item.FullName }
            Remove-Item -LiteralPath $directory.FullName
        }
    }
    $installRoot = [IO.Path]::GetFullPath($InstallDirectory)
    Get-ChildItem -LiteralPath $installRoot -File | Where-Object { $_.Name -match '^screen-time\.previous-\d{8}-\d{6}\.exe$' } | Sort-Object Name -Descending | Select-Object -Skip 2 | ForEach-Object {
        if ([IO.Path]::GetDirectoryName($_.FullName) -eq $installRoot -and -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)) { Remove-Item -LiteralPath $_.FullName }
    }
}
