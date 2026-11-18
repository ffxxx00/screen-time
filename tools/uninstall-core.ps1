function Uninstall-AppFiles {
    param(
        [Parameter(Mandatory)][string]$InstallDirectory,
        [Parameter(Mandatory)][string[]]$ShortcutDirectories,
        [Parameter(Mandatory)][string]$RunKeyPath,
        [Parameter(Mandatory)][string]$UninstallKeyPath,
        [scriptblock]$StopApp = {
            param($executable)
            $processes = @(Get-Process -Name 'screen-time' -ErrorAction SilentlyContinue |
                Where-Object { $_.Path -and $_.Path -ieq $executable })
            if (-not $processes.Count) { return }
            $request = Start-Process -FilePath $executable -ArgumentList '--quit' -WindowStyle Hidden -PassThru -Wait
            if ($request.ExitCode -ne 0) { throw 'No se pudo solicitar la salida de Screen Time.' }
            foreach ($process in $processes) {
                if (-not $process.WaitForExit(10000)) { throw 'Screen Time no se cerró; usa Salir en la bandeja y reintenta.' }
            }
        }
    )
    $root = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
    if ([IO.Path]::GetFileName($root) -ne 'Screen Time' -or
        [IO.Path]::GetFileName([IO.Path]::GetDirectoryName($root)) -ne 'Programs') {
        throw "Ruta de instalación inesperada: $root"
    }
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { return }
    if ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "La instalación es un enlace: $root"
    }
    $exe = Join-Path $root 'screen-time.exe'
    & $StopApp $exe

    $shell = New-Object -ComObject WScript.Shell
    foreach ($directory in $ShortcutDirectories) {
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) { continue }
        Get-ChildItem -LiteralPath $directory -Filter '*.lnk' -File | ForEach-Object {
            if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { return }
            try {
                $target = $shell.CreateShortcut($_.FullName).TargetPath
                if ($target -and [IO.Path]::GetFullPath($target) -ieq $exe) { Remove-Item -LiteralPath $_.FullName }
            } catch { throw "No se pudo revisar o retirar el acceso directo $($_.FullName): $_" }
        }
    }
    foreach ($name in @('screen-time.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt','uninstall-user.ps1','uninstall-core.ps1')) {
        $path = Join-Path $root $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
        if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Archivo de instalación es un enlace: $path" }
        Remove-Item -LiteralPath $path
    }
    $docs = Join-Path $root 'docs'
    if (Test-Path -LiteralPath $docs -PathType Container) {
        if ((Get-Item -LiteralPath $docs).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Documentación es un enlace: $docs" }
        foreach ($name in @('ARQUITECTURA.md','VALIDACION.md','DEPENDENCIAS.md','PUBLICAR.md')) {
            $path = Join-Path $docs $name
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
            if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Archivo de documentación es un enlace: $path" }
            Remove-Item -LiteralPath $path
        }
        if (-not @(Get-ChildItem -LiteralPath $docs -Force).Count) { Remove-Item -LiteralPath $docs }
    }
    $webViewCache = [IO.Path]::GetFullPath((Join-Path $root 'screen-time.exe.WebView2'))
    if ([IO.Path]::GetDirectoryName($webViewCache) -eq $root -and
        [IO.Path]::GetFileName($webViewCache) -eq 'screen-time.exe.WebView2' -and
        (Test-Path -LiteralPath $webViewCache -PathType Container)) {
        if ((Get-Item -LiteralPath $webViewCache).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Caché de WebView2 es un enlace: $webViewCache" }
        if (Get-ChildItem -LiteralPath $webViewCache -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | Select-Object -First 1) { throw "Caché de WebView2 contiene un enlace: $webViewCache" }
        try { Remove-Item -LiteralPath $webViewCache -Recurse -Force }
        catch { Write-Warning "La caché de WebView2 quedó en $($webViewCache): $_" }
    }
    $runValue = (Get-ItemProperty -Path $RunKeyPath -Name 'Screen Time' -ErrorAction SilentlyContinue).'Screen Time'
    if ($runValue -and $runValue.Trim('"') -ieq $exe) {
        Remove-ItemProperty -Path $RunKeyPath -Name 'Screen Time'
    }
    $registered = Get-ItemProperty -Path $UninstallKeyPath -ErrorAction SilentlyContinue
    if ($registered -and $registered.InstallLocation -ieq $root -and $registered.DisplayName -eq 'Screen Time') {
        Remove-Item -Path $UninstallKeyPath -Force
    }
    if (-not @(Get-ChildItem -LiteralPath $root -Force).Count) { Remove-Item -LiteralPath $root }
}
