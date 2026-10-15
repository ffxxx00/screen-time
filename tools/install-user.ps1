param([string]$SourceDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'install-core.ps1')
$projectRoot = Split-Path $PSScriptRoot
if (-not $SourceDirectory) { $SourceDirectory = if (Test-Path -LiteralPath (Join-Path $projectRoot 'screen-time.exe')) { $projectRoot } else { Join-Path $projectRoot 'target\x86_64-pc-windows-msvc\release' } }
$sourceExe = Join-Path $SourceDirectory 'screen-time.exe'
$legacyInstallRoot = Join-Path $env:LOCALAPPDATA 'Programs\HorariosTracker'
$previousInstallRoot = Join-Path $env:LOCALAPPDATA 'Programs\Tiempo en pantalla'
$installRoot = Join-Path $env:LOCALAPPDATA 'Programs\Screen Time'
$installedExe = Join-Path $installRoot 'screen-time.exe'
$legacyExe = Join-Path $legacyInstallRoot 'horarios-tracker.exe'
$dataRoot = Join-Path $env:LOCALAPPDATA 'HorariosTracker'
if (-not (Test-Path -LiteralPath $sourceExe)) { throw 'Primero compila la version Release.' }
if (-not (Test-WebView2Runtime)) {
    throw 'Falta Microsoft Edge WebView2 Runtime. Instálalo desde https://developer.microsoft.com/microsoft-edge/webview2/ y vuelve a ejecutar Instalar.cmd.'
}
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class TrackerInstall {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr param);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr param);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int count);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] public static extern bool PostThreadMessageW(uint thread,uint msg,UIntPtr wp,IntPtr lp);
 public static bool HasLifecycleWindow(uint target) {
  bool found=false;
  EnumWindows((hwnd,param)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);
   if(pid==target){var name=new StringBuilder(256);GetClassNameW(hwnd,name,256);if(name.ToString()=="HorariosTracker.Lifecycle"){found=true;return false;}}return true;
  },IntPtr.Zero);return found;
 }
 public static uint FindThread(uint target) {
  uint found=0;
  EnumWindows((hwnd,param)=>{uint pid;uint thread=GetWindowThreadProcessId(hwnd,out pid);
   if(pid==target){var name=new StringBuilder(256);GetClassNameW(hwnd,name,256);if(name.ToString()=="Tao Thread Event Target"){found=thread;return false;}}return true;
  },IntPtr.Zero);return found;
 }
}
"@
function Start-CheckedTracker {
    param([string]$Executable, [string]$Directory)
    $process = Start-Process -FilePath $Executable -WorkingDirectory $Directory -WindowStyle Hidden -PassThru
    try {
        $ready = $false
        for ($attempt = 0; $attempt -lt 50; $attempt++) {
            $process.Refresh()
            if ($process.HasExited) { throw "El tracker terminó al iniciar (código $($process.ExitCode))." }
            if ([TrackerInstall]::HasLifecycleWindow([uint32]$process.Id)) { $ready = $true; break }
            Start-Sleep -Milliseconds 200
        }
        if (-not $ready) { throw 'El tracker no creó su ventana de sesión dentro de 10 segundos.' }
        if ($process.WaitForExit(500)) { throw "El tracker terminó tras iniciar (código $($process.ExitCode))." }
        return $process
    } catch {
        $process.Refresh()
        if (-not $process.HasExited) {
            $thread = [TrackerInstall]::FindThread([uint32]$process.Id)
            if ($thread -ne 0) { [void][TrackerInstall]::PostThreadMessageW($thread,0x12,[UIntPtr]::Zero,[IntPtr]::Zero) }
            if (-not $process.WaitForExit(3000)) { Stop-Process -Id $process.Id -Force; [void]$process.WaitForExit(3000) }
        }
        throw
    }
}
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Screen Time'
$uninstallFields = @('DisplayName','DisplayVersion','Publisher','InstallLocation','DisplayIcon','UninstallString','NoModify','NoRepair')
$hadUninstallKey = Test-Path $uninstallKey
$oldUninstall = @{}
if ($hadUninstallKey) {
 $properties = Get-ItemProperty -Path $uninstallKey
 foreach ($name in $uninstallFields) { $oldUninstall[$name] = $properties.$name }
}
$startupEnabled = $null -ne (Get-ItemProperty -Path $runKey -Name HorariosTracker -ErrorAction SilentlyContinue)
$allowedDirs = @((Split-Path $sourceExe), $installRoot, $previousInstallRoot, $legacyInstallRoot)
$running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'horarios-tracker*' -or $_.ProcessName -like 'tiempo-en-pantalla*' -or $_.ProcessName -like 'screen-time*' } | Where-Object { $_.Path -and $allowedDirs -contains (Split-Path $_.Path) })
$restartExe = if ($running.Count) { $running[0].Path } else { $null }
$payload = Join-Path ([IO.Path]::GetTempPath()) ('screen-time-payload-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $payload | Out-Null
$linkPaths = @((Join-Path ([Environment]::GetFolderPath('Desktop')) 'Screen Time.lnk'), (Join-Path ([Environment]::GetFolderPath('Programs')) 'Screen Time.lnk'))
$linkBackups = @{}
foreach ($link in $linkPaths) { $linkBackups[$link] = if (Test-Path -LiteralPath $link) { [IO.File]::ReadAllBytes($link) } else { $null } }
$oldStartup = (Get-ItemProperty -Path $runKey -Name HorariosTracker -ErrorAction SilentlyContinue).HorariosTracker
try {
 Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $payload 'screen-time.exe')
 foreach ($name in @('README.md','LICENSE','PRIVACY.md')) { Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $payload }
 foreach ($name in @('uninstall-user.ps1','uninstall-core.ps1')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $payload }
 Copy-Item -LiteralPath (Join-Path $projectRoot 'docs') -Destination $payload -Recurse
 $chartLicense = Join-Path $projectRoot 'CHARTJS-LICENSE.txt'
 if (-not (Test-Path -LiteralPath $chartLicense)) { $chartLicense = Join-Path $projectRoot 'assets\CHARTJS-LICENSE.txt' }
 Copy-Item -LiteralPath $chartLicense -Destination (Join-Path $payload 'CHARTJS-LICENSE.txt')
foreach ($process in $running) {
 $thread = [TrackerInstall]::FindThread([uint32]$process.Id)
 if ($thread -eq 0 -or -not [TrackerInstall]::PostThreadMessageW($thread,0x12,[UIntPtr]::Zero,[IntPtr]::Zero)) { throw 'No se pudo cerrar ordenadamente el tracker. Usa Salir en la bandeja y vuelve a instalar.' }
 if (-not $process.WaitForExit(10000)) { throw 'El tracker no termino; no se reemplazo el ejecutable.' }
}
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
if (Test-Path -LiteralPath $dataRoot) {
 $backup = Join-Path $dataRoot "backups\pre-install-$stamp"
 New-Item -ItemType Directory -Path $backup -Force | Out-Null
 Get-ChildItem -LiteralPath $dataRoot -File -Filter 'app_usage.*' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $backup }
}
 Install-AppFiles -SourceDirectory $payload -InstallDirectory $installRoot -AfterInstall {
  $shell = New-Object -ComObject WScript.Shell
  foreach ($linkPath in $linkPaths) {
   $shortcut = $shell.CreateShortcut($linkPath)
   $shortcut.TargetPath = $installedExe; $shortcut.Arguments = '--dashboard'
   $shortcut.WorkingDirectory = $installRoot; $shortcut.IconLocation = "$installedExe,0"
   $shortcut.Description = 'Screen Time'; $shortcut.Save()
  }
  if ($startupEnabled) { New-ItemProperty -Path $runKey -Name HorariosTracker -Value ('"' + $installedExe + '"') -PropertyType String -Force | Out-Null }
  New-Item -Path $uninstallKey -Force | Out-Null
  $registration = @{
   DisplayName = 'Screen Time'
   DisplayVersion = (Get-Item -LiteralPath $installedExe).VersionInfo.ProductVersion
   Publisher = 'Screen Time contributors'
   InstallLocation = $installRoot
   DisplayIcon = "$installedExe,0"
   UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $installRoot 'uninstall-user.ps1')`""
   NoModify = 1
   NoRepair = 1
  }
  foreach ($name in $uninstallFields) {
   $type = if ($name -in @('NoModify','NoRepair')) { 'DWord' } else { 'String' }
   New-ItemProperty -Path $uninstallKey -Name $name -Value $registration[$name] -PropertyType $type -Force | Out-Null
  }
  $script:installedProcess = Start-CheckedTracker -Executable $installedExe -Directory $installRoot
 }
} catch {
 foreach ($link in $linkPaths) {
  if ($null -ne $linkBackups[$link]) { [IO.File]::WriteAllBytes($link, $linkBackups[$link]) }
  elseif (Test-Path -LiteralPath $link) { Remove-Item -LiteralPath $link }
 }
 if ($null -ne $oldStartup) { New-ItemProperty -Path $runKey -Name HorariosTracker -Value $oldStartup -PropertyType String -Force | Out-Null }
 elseif (Get-ItemProperty -Path $runKey -Name HorariosTracker -ErrorAction SilentlyContinue) { Remove-ItemProperty -Path $runKey -Name HorariosTracker }
 if ($hadUninstallKey) {
  New-Item -Path $uninstallKey -Force | Out-Null
  foreach ($name in $uninstallFields) {
   if ($null -eq $oldUninstall[$name]) { Remove-ItemProperty -Path $uninstallKey -Name $name -ErrorAction SilentlyContinue }
   else {
    $type = if ($name -in @('NoModify','NoRepair')) { 'DWord' } else { 'String' }
    New-ItemProperty -Path $uninstallKey -Name $name -Value $oldUninstall[$name] -PropertyType $type -Force | Out-Null
   }
  }
 } elseif (Test-Path $uninstallKey) { Remove-Item -Path $uninstallKey -Force }
 if ($restartExe -and (Test-Path -LiteralPath $restartExe)) { try { Start-CheckedTracker -Executable $restartExe -Directory (Split-Path $restartExe) | Out-Null } catch { Write-Warning "No se pudo reiniciar la versión anterior: $_" } }
 throw
} finally {
 $resolvedPayload = [IO.Path]::GetFullPath($payload)
 if ([IO.Path]::GetDirectoryName($resolvedPayload).TrimEnd('\') -eq [IO.Path]::GetTempPath().TrimEnd('\') -and [IO.Path]::GetFileName($resolvedPayload).StartsWith('screen-time-payload-')) {
  Remove-Item -LiteralPath $resolvedPayload -Recurse -Force -ErrorAction SilentlyContinue
 }
}
try { Remove-LegacyArtifacts -LegacyInstallDirectory $legacyInstallRoot -ShortcutDirectories @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs')) } catch { Write-Warning "Instalación completada; revisa la versión anterior manualmente: $_" }
try { Remove-PreviousProductArtifacts -PreviousInstallDirectory $previousInstallRoot -ShortcutDirectories @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs')) } catch { Write-Warning "Instalación completada; revisa Tiempo en pantalla manualmente: $_" }
$installedProcess | Select-Object Id,Path
try { Remove-OldAppBackups -DataDirectory $dataRoot -InstallDirectory $installRoot } catch { Write-Warning "Instalacion completada; no se pudieron limpiar copias antiguas: $_" }
Write-Output "Instalado en: $installRoot"
Write-Output "Inicio automatico conservado: $startupEnabled"
