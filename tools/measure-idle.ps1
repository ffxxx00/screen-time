param([int]$Seconds = 30)
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 5 -or $Seconds -gt 600) { throw 'Seconds debe estar entre 5 y 600.' }
$processes = @(Get-Process -Name 'screen-time' -ErrorAction Stop)
if ($processes.Count -ne 1) { throw 'La medición requiere una sola instancia del tracker.' }
$id = $processes[0].Id
$samples = [Collections.Generic.List[object]]::new()
$clock = [Diagnostics.Stopwatch]::StartNew()
for ($i = 0; $i -le $Seconds; $i++) {
    $process = Get-Process -Id $id -ErrorAction Stop
    $samples.Add([pscustomobject]@{
        Elapsed = $clock.Elapsed.TotalSeconds
        CpuSeconds = $process.TotalProcessorTime.TotalSeconds
        WorkingSetMB = $process.WorkingSet64 / 1MB
        PrivateMB = $process.PrivateMemorySize64 / 1MB
    })
    if ($i -lt $Seconds) { Start-Sleep -Seconds 1 }
}
$first = $samples[0]
$last = $samples[$samples.Count - 1]
$cpu = 100 * ($last.CpuSeconds - $first.CpuSeconds) / ($last.Elapsed - $first.Elapsed) / [Environment]::ProcessorCount
[pscustomobject]@{
    ProcessId = $id
    Seconds = [math]::Round($last.Elapsed - $first.Elapsed, 1)
    CpuPercentAllCores = [math]::Round($cpu, 3)
    WorkingSetMinMB = [math]::Round(($samples.WorkingSetMB | Measure-Object -Minimum).Minimum, 2)
    WorkingSetMaxMB = [math]::Round(($samples.WorkingSetMB | Measure-Object -Maximum).Maximum, 2)
    PrivateMinMB = [math]::Round(($samples.PrivateMB | Measure-Object -Minimum).Minimum, 2)
    PrivateMaxMB = [math]::Round(($samples.PrivateMB | Measure-Object -Maximum).Maximum, 2)
}
