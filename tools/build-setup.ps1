param([string]$OutputDirectory = (Join-Path $PSScriptRoot '..\dist'))
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot
$version = (Select-String -LiteralPath (Join-Path $project 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$output = [IO.Path]::GetFullPath($OutputDirectory)
$archive = Join-Path $output "screen-time-$version-windows-x64.zip"
if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) { throw 'Primero genera el ZIP mediante package-release.ps1.' }
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
if (-not (Test-Path -LiteralPath $compiler)) { throw 'Falta el compilador de .NET Framework incluido en Windows 10/11 x64.' }
$stage = Join-Path $output ('setup-build-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $stage 'payload.sha256'), $hash, [Text.Encoding]::ASCII)
    $assembly = '[assembly:System.Reflection.AssemblyTitle("Screen Time Setup")][assembly:System.Reflection.AssemblyProduct("Screen Time")][assembly:System.Reflection.AssemblyVersion("' + $version + '.0")][assembly:System.Reflection.AssemblyFileVersion("' + $version + '.0")]'
    [IO.File]::WriteAllText((Join-Path $stage 'version.cs'), $assembly, [Text.Encoding]::UTF8)
    $setup = Join-Path $output "screen-time-$version-setup-x64.exe"
    $compilerArgs = @('/nologo','/target:winexe','/platform:x64','/optimize+',('/out:' + $setup),('/win32icon:' + (Join-Path $project 'assets\app.ico')),('/win32manifest:' + (Join-Path $project 'installer\app.manifest')),('/resource:' + $archive + ',ScreenTime.Payload'),('/resource:' + (Join-Path $stage 'payload.sha256') + ',ScreenTime.Checksum'),'/reference:System.Windows.Forms.dll','/reference:System.Drawing.dll','/reference:System.IO.Compression.dll','/reference:System.IO.Compression.FileSystem.dll',(Join-Path $project 'installer\Setup.cs'),(Join-Path $stage 'version.cs'))
    & $compiler @compilerArgs
    if ($LASTEXITCODE -ne 0) { throw 'No se pudo compilar el instalador.' }
    $verify = Start-Process -FilePath $setup -ArgumentList '--verify' -WindowStyle Hidden -PassThru -Wait
    if ($verify.ExitCode -ne 0) { throw 'El instalador no superó la extracción y verificación de su paquete.' }
    $setupHash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(($setup + '.sha256'), "$setupHash  $([IO.Path]::GetFileName($setup))`n", [Text.Encoding]::ASCII)
    Write-Output $setup
} finally {
    $resolved = [IO.Path]::GetFullPath($stage)
    if ([IO.Path]::GetDirectoryName($resolved) -eq $output -and [IO.Path]::GetFileName($resolved).StartsWith('setup-build-')) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
