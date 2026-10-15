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

function Remove-LegacyArtifacts {
    param([string]$LegacyInstallDirectory, [string[]]$ShortcutDirectories)
    $legacyRoot = [IO.Path]::GetFullPath($LegacyInstallDirectory).TrimEnd('\')
    if ([IO.Path]::GetFileName($legacyRoot) -ne 'HorariosTracker' -or [IO.Path]::GetFileName([IO.Path]::GetDirectoryName($legacyRoot)) -ne 'Programs') {
        throw "Ruta de instalación antigua inesperada: $legacyRoot"
    }
    $legacyExe = Join-Path $legacyRoot 'horarios-tracker.exe'
    $shell = New-Object -ComObject WScript.Shell
    foreach ($directory in $ShortcutDirectories) {
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) { continue }
        Get-ChildItem -LiteralPath $directory -Filter '*.lnk' -File | ForEach-Object {
            if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { return }
            try {
                $target = $shell.CreateShortcut($_.FullName).TargetPath
                if ($target -and [IO.Path]::GetFullPath($target) -ieq $legacyExe) { Remove-Item -LiteralPath $_.FullName }
            } catch { Write-Warning "No se pudo revisar el acceso directo $($_.FullName): $_" }
        }
    }
    if (Test-Path -LiteralPath $legacyRoot -PathType Container) {
        $folder = Get-Item -LiteralPath $legacyRoot
        if ($folder.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "La instalación antigua es un enlace: $legacyRoot" }
        if (Test-Path -LiteralPath $legacyExe -PathType Leaf) {
            $exe = Get-Item -LiteralPath $legacyExe
            if ($exe.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "El ejecutable antiguo es un enlace: $legacyExe" }
            Remove-Item -LiteralPath $legacyExe
        }
        if (-not @(Get-ChildItem -LiteralPath $legacyRoot -Force).Count) { Remove-Item -LiteralPath $legacyRoot }
    }
}

function Remove-PreviousProductArtifacts {
    param([string]$PreviousInstallDirectory, [string[]]$ShortcutDirectories)
    $previousRoot = [IO.Path]::GetFullPath($PreviousInstallDirectory).TrimEnd('\')
    if ([IO.Path]::GetFileName($previousRoot) -ne 'Tiempo en pantalla' -or [IO.Path]::GetFileName([IO.Path]::GetDirectoryName($previousRoot)) -ne 'Programs') {
        throw "Ruta de instalación anterior inesperada: $previousRoot"
    }
    $previousExe = Join-Path $previousRoot 'tiempo-en-pantalla.exe'
    $shell = New-Object -ComObject WScript.Shell
    foreach ($directory in $ShortcutDirectories) {
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) { continue }
        Get-ChildItem -LiteralPath $directory -Filter '*.lnk' -File | ForEach-Object {
            if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { return }
            try {
                $target = $shell.CreateShortcut($_.FullName).TargetPath
                if ($target -and [IO.Path]::GetFullPath($target) -ieq $previousExe) { Remove-Item -LiteralPath $_.FullName }
            } catch { Write-Warning "No se pudo revisar el acceso directo $($_.FullName): $_" }
        }
    }
    if (Test-Path -LiteralPath $previousRoot -PathType Container) {
        $folder = Get-Item -LiteralPath $previousRoot
        if ($folder.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "La instalación anterior es un enlace: $previousRoot" }
        $knownFiles = @('tiempo-en-pantalla.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt','CHANGELOG.md')
        $knownFiles += @(Get-ChildItem -LiteralPath $previousRoot -File | Where-Object { $_.Name -match '^tiempo-en-pantalla\.previous-\d{8}-\d{6}\.exe$' } | Select-Object -ExpandProperty Name)
        foreach ($name in $knownFiles) {
            $path = Join-Path $previousRoot $name
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
            $item = Get-Item -LiteralPath $path
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Archivo anterior es un enlace: $path" }
            Remove-Item -LiteralPath $path
        }
        $docs = Join-Path $previousRoot 'docs'
        if (Test-Path -LiteralPath $docs -PathType Container) {
            if ((Get-Item -LiteralPath $docs).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Documentación anterior es un enlace: $docs" }
            foreach ($name in @('ARQUITECTURA.md','VALIDACION.md','DEPENDENCIAS.md','PUBLICAR.md')) {
                $path = Join-Path $docs $name
                if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
                if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Archivo anterior es un enlace: $path" }
                Remove-Item -LiteralPath $path
            }
            if (-not @(Get-ChildItem -LiteralPath $docs -Force).Count) { Remove-Item -LiteralPath $docs }
        }
        $oldWebViewCache = [IO.Path]::GetFullPath((Join-Path $previousRoot 'tiempo-en-pantalla.exe.WebView2'))
        if ([IO.Path]::GetDirectoryName($oldWebViewCache) -eq $previousRoot -and
            [IO.Path]::GetFileName($oldWebViewCache) -eq 'tiempo-en-pantalla.exe.WebView2' -and
            (Test-Path -LiteralPath $oldWebViewCache -PathType Container)) {
            if ((Get-Item -LiteralPath $oldWebViewCache).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Caché anterior es un enlace: $oldWebViewCache" }
            if (Get-ChildItem -LiteralPath $oldWebViewCache -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) { throw "La caché anterior contiene un enlace: $oldWebViewCache" }
            Remove-Item -LiteralPath $oldWebViewCache -Recurse -Force
        }
        if (-not @(Get-ChildItem -LiteralPath $previousRoot -Force).Count) { Remove-Item -LiteralPath $previousRoot }
    }
}
