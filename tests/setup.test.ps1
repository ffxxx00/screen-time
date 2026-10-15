param([string]$SetupPath)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot
if (-not $SetupPath) {
    $version = (Select-String -LiteralPath (Join-Path $project 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
    $SetupPath = Join-Path $project "dist\screen-time-$version-setup-x64.exe"
}
$setup = [IO.Path]::GetFullPath($SetupPath)
$expected = ([IO.File]::ReadAllText(($setup + '.sha256')) -split '\s+')[0]
if ((Get-FileHash -LiteralPath $setup).Hash.ToLowerInvariant() -ne $expected) { throw 'Checksum del instalador incorrecto.' }
$before = @(Get-ChildItem -LiteralPath ([IO.Path]::GetTempPath()) -Directory -Filter 'screen-time-setup-*' | ForEach-Object Name)
$verify = Start-Process -FilePath $setup -ArgumentList '--verify' -PassThru -Wait -WindowStyle Hidden
if ($verify.ExitCode -ne 0) { throw 'El EXE no pudo verificar y extraer el paquete incrustado.' }
$after = @(Get-ChildItem -LiteralPath ([IO.Path]::GetTempPath()) -Directory -Filter 'screen-time-setup-*' | ForEach-Object Name)
if (@($after | Where-Object { $before -notcontains $_ }).Count) { throw 'La verificación dejó archivos temporales.' }
$invalid = Start-Process -FilePath $setup -ArgumentList '--invalid' -PassThru -Wait -WindowStyle Hidden
if ($invalid.ExitCode -ne 2) { throw 'El EXE no rechazó un argumento no reconocido.' }
$damaged = Join-Path ([IO.Path]::GetTempPath()) ('screen-time-damaged-setup-' + [guid]::NewGuid().ToString('N') + '.exe')
try {
    $bytes = [IO.File]::ReadAllBytes($setup)
    $zipOffset = [Text.Encoding]::ASCII.GetString($bytes).IndexOf(('PK' + [char]3 + [char]4), [StringComparison]::Ordinal)
    if ($zipOffset -lt 0) { throw 'No se encontró el recurso ZIP para la prueba de integridad.' }
    $bytes[$zipOffset + 35] = $bytes[$zipOffset + 35] -bxor 1
    [IO.File]::WriteAllBytes($damaged, $bytes)
    $rejected = Start-Process -FilePath $damaged -ArgumentList '--verify' -PassThru -Wait -WindowStyle Hidden
    if ($rejected.ExitCode -ne 1) { throw 'El instalador aceptó un paquete alterado.' }
} finally { if (Test-Path -LiteralPath $damaged -PathType Leaf) { Remove-Item -LiteralPath $damaged } }
$stream = [IO.File]::OpenRead($setup)
try {
    $reader = New-Object IO.BinaryReader($stream)
    $stream.Position = 0x3c
    $pe = $reader.ReadInt32()
    $stream.Position = $pe + 24 + 68
    if ($reader.ReadUInt16() -ne 2) { throw 'El instalador no usa el subsistema gráfico de Windows.' }
} finally { $stream.Dispose() }
Write-Output 'Setup EXE: checksum, extracción, integridad, limpieza temporal, argumentos y subsistema gráfico aprobados.'
