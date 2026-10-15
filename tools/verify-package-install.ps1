param([string]$Archive)
$ErrorActionPreference = 'Stop'
if (-not $Archive) {
    $manifest = Join-Path $PSScriptRoot '..\Cargo.toml'
    $version = (Select-String -LiteralPath $manifest -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
    $Archive = Join-Path $PSScriptRoot "..\dist\screen-time-$version-windows-x64.zip"
}
$archivePath = [IO.Path]::GetFullPath($Archive)
$expected = ((Get-Content -LiteralPath ($archivePath + '.sha256') -Raw).Split(' ',[StringSplitOptions]::RemoveEmptyEntries))[0]
$actual = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw 'Checksum ZIP incorrecto.' }
$stage = Join-Path ([IO.Path]::GetTempPath()) ('screen-time-package-check-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    Expand-Archive -LiteralPath $archivePath -DestinationPath $stage
    & (Join-Path $stage 'tools\install-user.ps1') -SourceDirectory $stage
    $installed = Join-Path $env:LOCALAPPDATA 'Programs\Screen Time\screen-time.exe'
    if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath (Join-Path $stage 'screen-time.exe')).Hash) {
        throw 'El ejecutable instalado difiere del ZIP.'
    }
    Write-Output 'ZIP SHA-256, instalación desde ZIP y ejecutable instalado: correctos.'
} finally {
    $resolved = [IO.Path]::GetFullPath($stage)
    if ([IO.Path]::GetDirectoryName($resolved).TrimEnd('\') -eq [IO.Path]::GetTempPath().TrimEnd('\') -and [IO.Path]::GetFileName($resolved).StartsWith('screen-time-package-check-')) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
