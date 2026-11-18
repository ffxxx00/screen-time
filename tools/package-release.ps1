param([string]$OutputDirectory = (Join-Path $PSScriptRoot '..\dist'))
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot
$version = (Select-String -LiteralPath (Join-Path $project 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$source = Join-Path $project 'target\x86_64-pc-windows-msvc\release\screen-time.exe'
if (-not (Test-Path -LiteralPath $source)) { throw 'Compila Release antes de empaquetar.' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $output -Force | Out-Null
$stage = Join-Path $output ('package-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $stage 'tools') -Force | Out-Null
try {
 Copy-Item -LiteralPath $source -Destination $stage
 foreach ($name in @('README.md','LICENSE','PRIVACY.md','CHANGELOG.md')) { Copy-Item -LiteralPath (Join-Path $project $name) -Destination $stage }
 New-Item -ItemType Directory -Path (Join-Path $stage 'docs') -Force | Out-Null
 foreach ($name in @('ARQUITECTURA.md','VALIDACION.md','DEPENDENCIAS.md','PUBLICAR.md')) { Copy-Item -LiteralPath (Join-Path $project "docs\$name") -Destination (Join-Path $stage 'docs') }
 Copy-Item -LiteralPath (Join-Path $project 'assets\CHARTJS-LICENSE.txt') -Destination $stage
 foreach ($name in @('install-user.ps1','install-core.ps1','uninstall-user.ps1','uninstall-core.ps1')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $stage 'tools') }
 [IO.File]::WriteAllText((Join-Path $stage 'Instalar.cmd'), "@echo off`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"%~dp0tools\install-user.ps1`"`r`nif errorlevel 1 pause`r`n", [Text.Encoding]::ASCII)
 $archive = Join-Path $output "screen-time-$version-windows-x64.zip"
 Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force
 $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
 [IO.File]::WriteAllText(($archive + '.sha256'), "$hash  $([IO.Path]::GetFileName($archive))`n", [Text.Encoding]::ASCII)
 Write-Output $archive
} finally {
 if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($stage)) -eq $output -and [IO.Path]::GetFileName($stage).StartsWith('package-')) { Remove-Item -LiteralPath $stage -Recurse -Force }
}
