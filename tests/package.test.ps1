param(
    [string]$ArchivePath
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\tools\install-core.ps1')
$project = Split-Path $PSScriptRoot
if (-not $ArchivePath) {
    $version = (Select-String -LiteralPath (Join-Path $project 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
    $ArchivePath = Join-Path $project "dist\screen-time-$version-windows-x64.zip"
}
$archive = [IO.Path]::GetFullPath($ArchivePath)
$checksum = "$archive.sha256"
if (-not (Test-Path -LiteralPath $archive -PathType Leaf) -or -not (Test-Path -LiteralPath $checksum -PathType Leaf)) {
    throw 'Faltan el ZIP Release o su checksum. Ejecuta package-release.ps1 antes de este test.'
}
$expected = ([IO.File]::ReadAllText($checksum) -split '\s+')[0].ToLowerInvariant()
$actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($expected -ne $actual) { throw 'El ZIP no coincide con su checksum SHA-256.' }

$root = Join-Path ([IO.Path]::GetTempPath()) ('screen-time-package-test-' + [guid]::NewGuid().ToString('N'))
$extracted = Join-Path $root 'extracted'
$payload = Join-Path $root 'payload'
$installed = Join-Path $root 'Programs\Screen Time'
New-Item -ItemType Directory -Path $extracted,$payload -Force | Out-Null
try {
    Expand-Archive -LiteralPath $archive -DestinationPath $extracted
    foreach ($name in @('screen-time.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt','Instalar.cmd')) {
        if (-not (Test-Path -LiteralPath (Join-Path $extracted $name) -PathType Leaf)) { throw "Falta en ZIP: $name" }
    }
    foreach ($name in @('install-user.ps1','install-core.ps1','uninstall-user.ps1','uninstall-core.ps1')) {
        if (-not (Test-Path -LiteralPath (Join-Path $extracted "tools\$name") -PathType Leaf)) { throw "Falta en ZIP: tools\$name" }
    }
    foreach ($name in @('ARQUITECTURA.md','VALIDACION.md','DEPENDENCIAS.md','PUBLICAR.md')) {
        if (-not (Test-Path -LiteralPath (Join-Path $extracted "docs\$name") -PathType Leaf)) { throw "Falta en ZIP: docs\$name" }
    }
    $cmd = [IO.File]::ReadAllText((Join-Path $extracted 'Instalar.cmd'))
    if ($cmd -notmatch [regex]::Escape('tools\install-user.ps1')) { throw 'Instalar.cmd no apunta al instalador incluido.' }

    foreach ($name in @('screen-time.exe','README.md','LICENSE','PRIVACY.md','CHARTJS-LICENSE.txt')) {
        Copy-Item -LiteralPath (Join-Path $extracted $name) -Destination $payload
    }
    Copy-Item -LiteralPath (Join-Path $extracted 'docs') -Destination $payload -Recurse
    foreach ($name in @('uninstall-user.ps1','uninstall-core.ps1')) {
        Copy-Item -LiteralPath (Join-Path $extracted "tools\$name") -Destination $payload
    }
    Install-AppFiles -SourceDirectory $payload -InstallDirectory $installed
    $installedHash = (Get-FileHash -LiteralPath (Join-Path $installed 'screen-time.exe') -Algorithm SHA256).Hash
    $packageHash = (Get-FileHash -LiteralPath (Join-Path $extracted 'screen-time.exe') -Algorithm SHA256).Hash
    if ($installedHash -ne $packageHash) { throw 'El ejecutable instalado no coincide con el del ZIP.' }
    if (Test-Path -LiteralPath (Join-Path $root 'HorariosTracker')) { throw 'La instalación de prueba tocó una carpeta de datos.' }
    Write-Output 'ZIP: checksum, contenido, instalador y copia temporal aprobados.'
} finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ([IO.Path]::GetDirectoryName($resolved).TrimEnd('\') -eq [IO.Path]::GetTempPath().TrimEnd('\') -and [IO.Path]::GetFileName($resolved).StartsWith('screen-time-package-test-')) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
