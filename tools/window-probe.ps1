# 窗口行为验收（只在本机跑：需要真实桌面与合成输入，CI 上不跑）
#
# 用法：pwsh tools/window-probe.ps1
#
# 覆盖四件事：拖头部能挪窗口、拉边缘能改大小、松手之后不再跟着鼠标、双击头部能最大化。
# 这几条全是截图看不出来的，也正是最近反复返工的地方。

$ErrorActionPreference = 'Stop'
Add-Type -Namespace Probe -Name U -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, System.IntPtr e);
[DllImport("user32.dll")] public static extern bool GetWindowRect(System.IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool SetWindowPos(System.IntPtr h, System.IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool BringWindowToTop(System.IntPtr h);
[DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
'@

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
$failures = 0

function Check([string]$name, [bool]$ok, [string]$detail) {
    if ($ok) { Write-Host ('  OK    ' + $name) }
    else { Write-Host ('  FAIL  ' + $name + '  ' + $detail); $script:failures++ }
}

function Rect($h) {
    $rect = New-Object Probe.U+RECT
    [void][Probe.U]::GetWindowRect($h, [ref]$rect)
    $rect
}

function Start-App {
    $base = Join-Path ([System.IO.Path]::GetTempPath()) ('notecevia-probe-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $base | Out-Null
    & cargo run -q -p notecevia-store --example make_fixture -- $base | Out-Null
    $env:NOTECEVIA_HOME = $base
    $exe = Join-Path $root 'target\debug\notecevia.exe'
    $process = Start-Process -FilePath $exe -ArgumentList '--home' -PassThru
    Start-Sleep -Seconds 3
    $process.Refresh()
    if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw '窗口没起来' }
    [void][Probe.U]::SetWindowPos($process.MainWindowHandle, [IntPtr](-2), 40, 40, 900, 600, 0x0004)
    [void][Probe.U]::BringWindowToTop($process.MainWindowHandle)
    [void][Probe.U]::SetForegroundWindow($process.MainWindowHandle)
    Start-Sleep -Milliseconds 700
    $process
}

function Stop-App($process) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 400
}

function Press([int]$x, [int]$y) {
    [void][Probe.U]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 250
    [Probe.U]::mouse_event(0x02, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
}

function Move-To([int]$x, [int]$y) {
    [void][Probe.U]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 60
}

function Release {
    [Probe.U]::mouse_event(0x04, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
}

# 两次点击必须挨得够近，否则应用看到的是两次单击，不是双击。
function Double-Click([int]$x, [int]$y) {
    [void][Probe.U]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 250
    for ($i = 0; $i -lt 2; $i++) {
        [Probe.U]::mouse_event(0x02, 0, 0, 0, [IntPtr]::Zero)
        [Probe.U]::mouse_event(0x04, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 90
    }
    Start-Sleep -Milliseconds 700
}

try {
    Write-Host '拖动：'
    $app = Start-App
    $before = Rect $app.MainWindowHandle
    Press ($before.Left + 500) ($before.Top + 16)
    for ($i = 1; $i -le 10; $i++) { Move-To ($before.Left + 500 - 5 * $i) ($before.Top + 16 - 4 * $i) }
    Release
    $after = Rect $app.MainWindowHandle
    $moved = [math]::Abs($after.Left - $before.Left) + [math]::Abs($after.Top - $before.Top)
    Check '拖头部窗口跟着走' ($moved -gt 30) ('只挪了 ' + $moved + ' 像素')
    Stop-App $app

    Write-Host '缩放：'
    $app = Start-App
    $before = Rect $app.MainWindowHandle
    $width = $before.Right - $before.Left
    Press ($before.Right - 4) ([int](($before.Top + $before.Bottom) / 2))
    for ($i = 1; $i -le 10; $i++) { Move-To ($before.Right - 4 + 5 * $i) ([int](($before.Top + $before.Bottom) / 2)) }
    Release
    $after = Rect $app.MainWindowHandle
    $grown = ($after.Right - $after.Left) - $width
    Check '拉右边缘能改大小' ($grown -gt 20) ('宽度只变了 ' + $grown)

    $size_before = ($after.Right - $after.Left)
    Move-To ($after.Right - 120) ($after.Top + 300)
    Move-To ($after.Left + 100) ($after.Top + 200)
    Start-Sleep -Milliseconds 500
    $still = Rect $app.MainWindowHandle
    Check '松手后不再跟着鼠标缩' ((($still.Right - $still.Left) -eq $size_before)) ('松手后又变了 ' + (($still.Right - $still.Left) - $size_before))
    Stop-App $app

    Write-Host '最大化：'
    $app = Start-App
    $rect = Rect $app.MainWindowHandle
    Double-Click ($rect.Left + 500) ($rect.Top + 16)
    $max = Rect $app.MainWindowHandle
    $wide = ($max.Right - $max.Left) -ge ([Probe.U]::GetSystemMetrics(0) - 40)
    Check '双击头部能最大化' $wide ('宽 ' + ($max.Right - $max.Left) + '，屏幕 ' + [Probe.U]::GetSystemMetrics(0))
    Stop-App $app
} finally {
    Pop-Location
}

if ($failures -gt 0) {
    Write-Host ''
    Write-Host ($failures.ToString() + ' 条失败')
    exit 1
}
Write-Host ''
Write-Host '全部通过'
exit 0
