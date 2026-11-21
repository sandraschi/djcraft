$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class DjWindowCapture {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);

    public static IntPtr FindMainWindow(uint targetPid) {
        IntPtr found = IntPtr.Zero;
        int bestArea = 0;
        EnumWindows((hWnd, lParam) => {
            uint procId;
            GetWindowThreadProcessId(hWnd, out procId);
            if (procId == targetPid && IsWindowVisible(hWnd)) {
                RECT r;
                if (GetWindowRect(hWnd, out r)) {
                    int w = r.Right - r.Left;
                    int h = r.Bottom - r.Top;
                    int area = w * h;
                    if (w > 400 && h > 300 && area > bestArea) {
                        bestArea = area;
                        found = hWnd;
                    }
                }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

$repoRoot = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repoRoot 'target\release\djcraft.exe'
if (-not (Test-Path $exe)) {
    $exe = Join-Path $repoRoot 'dist\djcraft.exe'
}
$outDir = Join-Path $repoRoot 'docs\images'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$shots = @(
    @{ Name = 'live-console-battle'; Args = @('--skin', 'battle') },
    @{ Name = 'booth-crate-split'; Args = @('--skin', 'battle', '--booth-crate') },
    @{ Name = 'live-console-shocking-pink-4deck'; Args = @('--skin', 'pink', '--four-deck') },
    @{ Name = 'daylight-booth'; Args = @('--skin', 'daylight') },
    @{ Name = 'crates-gig-prep'; Args = @('--skin', 'battle', '--crates') }
)

foreach ($s in $shots) {
    $outPng = Join-Path $outDir "$($s.Name).png"
    $proc = Start-Process -FilePath $exe -ArgumentList $s.Args -PassThru
    try {
        $hwnd = [IntPtr]::Zero
        for ($i = 0; $i -lt 30; $i++) {
            Start-Sleep -Milliseconds 250
            $hwnd = [DjWindowCapture]::FindMainWindow([uint32]$proc.Id)
            if ($hwnd -ne [IntPtr]::Zero) { break }
        }
        if ($hwnd -eq [IntPtr]::Zero) {
            throw "Could not find visible DJCraft window for $($s.Name)"
        }
        Start-Sleep -Milliseconds 1200
        [DjWindowCapture]::SetProcessDPIAware() | Out-Null
        $r = New-Object DjWindowCapture+RECT
        [DjWindowCapture]::GetWindowRect($hwnd, [ref]$r) | Out-Null
        $w = [Math]::Max(1, $r.Right - $r.Left)
        $h = [Math]::Max(1, $r.Bottom - $r.Top)
        $bmp = New-Object System.Drawing.Bitmap($w, $h)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $hdc = $g.GetHdc()
        $ok = [DjWindowCapture]::PrintWindow($hwnd, $hdc, 2)
        if (-not $ok) { [DjWindowCapture]::PrintWindow($hwnd, $hdc, 0) | Out-Null }
        $g.ReleaseHdc($hdc)
        $bmp.Save($outPng, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose()
        $bmp.Dispose()
        Write-Output "Captured $($s.Name).png ($w x $h, ok=$ok, bytes=$((Get-Item $outPng).Length))"
    } finally {
        if (-not $proc.HasExited) {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 250
        }
    }
}
