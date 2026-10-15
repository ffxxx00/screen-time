$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class TrackerSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr param);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr param);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int count);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint message, UIntPtr w, IntPtr l);
 public static IntPtr Dashboard(uint target) {
  IntPtr found=IntPtr.Zero;
  EnumWindows((hwnd,param)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);
   if(pid==target && IsWindowVisible(hwnd)) {var title=new StringBuilder(256);GetWindowTextW(hwnd,title,256);
    if(title.ToString()=="Screen Time"){found=hwnd;return false;}}return true;
  },IntPtr.Zero);return found;
 }
}
"@
$installed = Join-Path $env:LOCALAPPDATA 'Programs\Screen Time\screen-time.exe'
$running = @(Get-Process -Name 'screen-time' -ErrorAction Stop)
if ($running.Count -ne 1 -or $running[0].Path -ne $installed) { throw 'Se espera una sola instancia instalada.' }
$first = $running[0]
if ([TrackerSmoke]::Dashboard([uint32]$first.Id) -ne [IntPtr]::Zero) { throw 'Cierra el dashboard antes de esta prueba.' }
$second = Start-Process -FilePath $installed -ArgumentList '--dashboard' -PassThru
if (-not $second.WaitForExit(10000) -or $second.ExitCode -ne 0) { throw 'La segunda invocación no terminó correctamente.' }
$dashboard = [IntPtr]::Zero
for ($attempt = 0; $attempt -lt 100; $attempt++) {
    $dashboard = [TrackerSmoke]::Dashboard([uint32]$first.Id)
    if ($dashboard -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 200
}
if ($dashboard -eq [IntPtr]::Zero) { throw 'El dashboard no apareció en 20 segundos.' }
if (@(Get-Process -Name 'screen-time').Count -ne 1) { throw 'La segunda instancia quedó ejecutándose.' }
if (-not [TrackerSmoke]::PostMessageW($dashboard,0x10,[UIntPtr]::Zero,[IntPtr]::Zero)) { throw 'No se pudo cerrar el dashboard.' }
for ($attempt = 0; $attempt -lt 50; $attempt++) {
    if ([TrackerSmoke]::Dashboard([uint32]$first.Id) -eq [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 200
}
if ([TrackerSmoke]::Dashboard([uint32]$first.Id) -ne [IntPtr]::Zero) { throw 'El dashboard no se cerró en 10 segundos.' }
Write-Output 'Segunda invocación: una instancia, dashboard visible y cierre correcto.'
