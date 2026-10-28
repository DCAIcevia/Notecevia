# 窗口行为验收（只在本机跑：需要真实桌面与合成输入，CI 上不跑）
#
# 用法：pwsh tools/window-probe.ps1
#
# 覆盖七件事：拖头部能挪窗口、拉边缘能改大小、松手之后不再跟着鼠标、双击头部能最大化、
# 悬停最大化按钮弹出贴靠布局、单击图标弹出系统菜单、双击图标关窗。
# 这几条都是截图看不出来的窗口行为。
#
# 贴靠浮窗与图标菜单是 Shell 提供的行为：先用原装 Win32 程序 regedit 量一次对照能力，
# 本机 Shell 不提供时这两条打印 SKIP，不计入失败。

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -Namespace Probe -Name U -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, System.IntPtr e);
[DllImport("user32.dll")] public static extern bool GetWindowRect(System.IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool GetClientRect(System.IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool ClientToScreen(System.IntPtr h, ref POINT p);
[DllImport("user32.dll")] public static extern bool SetWindowPos(System.IntPtr h, System.IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool BringWindowToTop(System.IntPtr h);
[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
[DllImport("user32.dll")] public static extern System.IntPtr GetWindowLongPtrW(System.IntPtr h, int i);
[DllImport("user32.dll")] public static extern System.IntPtr SendMessage(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
[DllImport("user32.dll")] public static extern System.IntPtr FindWindow(string cls, string title);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, System.IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
[DllImport("user32.dll")] public static extern int GetClassName(System.IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, System.IntPtr extra);
public delegate bool EnumWindowsProc(System.IntPtr h, System.IntPtr l);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
'@

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
$failures = 0
$skipped = 0

function Check([string]$name, [bool]$ok, [string]$detail) {
    if ($ok) { Write-Host ('  OK    ' + $name) }
    else { Write-Host ('  FAIL  ' + $name + '  ' + $detail); $script:failures++ }
}

function Skip([string]$name, [string]$detail) {
    Write-Host ('  SKIP  ' + $name + '  ' + $detail)
    $script:skipped++
}

function Rect($h) {
    $rect = New-Object Probe.U+RECT
    [void][Probe.U]::GetWindowRect($h, [ref]$rect)
    $rect
}

function Client-Rect($h) {
    $rect = New-Object Probe.U+RECT
    [void][Probe.U]::GetClientRect($h, [ref]$rect)
    $origin = New-Object Probe.U+POINT
    [void][Probe.U]::ClientToScreen($h, [ref]$origin)
    [pscustomobject]@{
        Left = $origin.X
        Top = $origin.Y
        Width = $rect.Right - $rect.Left
        Height = $rect.Bottom - $rect.Top
    }
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

function Click([int]$x, [int]$y) {
    [void][Probe.U]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 200
    [Probe.U]::mouse_event(0x02, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    [Probe.U]::mouse_event(0x04, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
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

# 直接问窗口 WM_NCHITTEST：交给系统的按钮自己回答 HTSYSMENU(3) 与 HTMAXBUTTON(9)。
# 这样探针不用知道界面的布局，也能确认命中区确实装上了。
function Find-CaptionZones($h) {
    $client = Client-Rect $h
    $right = $client.Left + $client.Width - 1
    $bottom = $client.Top + [math]::Min(64, $client.Height - 1)
    $boxes = @{ 3 = $null; 9 = $null }
    for ($y = $client.Top + 1; $y -le $bottom; $y += 4) {
        for ($x = $client.Left + 1; $x -le $right; $x += 4) {
            $packed = [IntPtr]((((($y -band 0xFFFF) -shl 16)) -bor ($x -band 0xFFFF)))
            $hit = [int][Probe.U]::SendMessage($h, 0x0084, [IntPtr]::Zero, $packed)
            if ($hit -ne 3 -and $hit -ne 9) { continue }
            $box = $boxes[$hit]
            if ($null -eq $box) {
                $boxes[$hit] = [pscustomobject]@{ MinX = $x; MinY = $y; MaxX = $x; MaxY = $y }
            } else {
                if ($x -lt $box.MinX) { $box.MinX = $x }
                if ($y -lt $box.MinY) { $box.MinY = $y }
                if ($x -gt $box.MaxX) { $box.MaxX = $x }
                if ($y -gt $box.MaxY) { $box.MaxY = $y }
            }
        }
    }
    $zones = @{}
    foreach ($code in @(3, 9)) {
        $box = $boxes[$code]
        if ($null -ne $box) {
            $zones[$code] = [pscustomobject]@{
                X = [int](($box.MinX + $box.MaxX) / 2)
                Y = [int](($box.MinY + $box.MaxY) / 2)
            }
        }
    }
    $zones
}

# 可见顶层窗口的快照，键是句柄，值是类名。
function Snapshot-Windows {
    $script:snapshot = @{}
    $callback = [Probe.U+EnumWindowsProc]{
        param([IntPtr]$h, [IntPtr]$l)
        if ([Probe.U]::IsWindowVisible($h)) {
            $name = New-Object System.Text.StringBuilder 256
            [void][Probe.U]::GetClassName($h, $name, 256)
            $script:snapshot[$h.ToInt64()] = $name.ToString()
        }
        return $true
    }
    [void][Probe.U]::EnumWindows($callback, [IntPtr]::Zero)
    $script:snapshot
}

# 用原装 Win32 程序量一次本机 Shell 的能力：贴靠浮窗与标题栏图标菜单都是 Shell 画出来的，
# 机器不提供时应用侧无论怎么实现都不会出现，探针对应的检查就退成 SKIP。
function Test-ShellControls {
    $snap = $false
    $icon_menu = $false
    $focused = $false
    $process = $null
    try {
        $process = Start-Process regedit -PassThru
        Start-Sleep -Seconds 2
        $process.Refresh()
        $handle = $process.MainWindowHandle
        if ($handle -eq [IntPtr]::Zero) {
            $candidate = Get-Process regedit -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
            if ($null -ne $candidate) { $handle = $candidate.MainWindowHandle }
        }
        if ($handle -ne [IntPtr]::Zero) {
            [void][Probe.U]::SetWindowPos($handle, [IntPtr](-2), 40, 40, 900, 600, 0x0004)
            for ($i = 0; $i -lt 5; $i++) {
                [void][Probe.U]::BringWindowToTop($handle)
                [void][Probe.U]::SetForegroundWindow($handle)
                Start-Sleep -Milliseconds 250
                if ([Probe.U]::GetForegroundWindow() -eq $handle) { $focused = $true; break }
            }
            if ($focused) {
                $rect = Rect $handle
                # 原生最大化按钮：标题栏右端。
                Move-To 8 860
                Start-Sleep -Milliseconds 300
                $before = Snapshot-Windows
                [void][Probe.U]::SetCursorPos($rect.Right - 24, $rect.Top + 18)
                Start-Sleep -Milliseconds 150
                [void][Probe.U]::SetCursorPos($rect.Right - 23, $rect.Top + 18)
                Start-Sleep -Milliseconds 150
                [void][Probe.U]::SetCursorPos($rect.Right - 24, $rect.Top + 18)
                Start-Sleep -Milliseconds 900
                $during = Snapshot-Windows
                $appeared = @($during.Keys | Where-Object { -not $before.ContainsKey($_) })
                $snap = $appeared.Count -gt 0
                Move-To 8 860
                Start-Sleep -Milliseconds 400
                # 原生标题栏图标：单击应出 #32768 菜单。
                Click ($rect.Left + 16) ($rect.Top + 16)
                Start-Sleep -Milliseconds 500
                $icon_menu = [Probe.U]::FindWindow('#32768', $null) -ne [IntPtr]::Zero
                if ($icon_menu) {
                    [Probe.U]::keybd_event([byte]0x1B, [byte]0, 0, [IntPtr]::Zero)
                    [Probe.U]::keybd_event([byte]0x1B, [byte]0, 0x0002, [IntPtr]::Zero)
                    Start-Sleep -Milliseconds 300
                }
            }
        }
    } finally {
        if ($null -ne $process) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Milliseconds 400
    }
    [pscustomobject]@{ Snap = $snap; IconMenu = $icon_menu; Focused = $focused }
}

try {
    Write-Host '对照能力：'
    $cap = Test-ShellControls
    Write-Host ('  INFO  对照程序 regedit 前台=' + $cap.Focused + '，贴靠浮窗=' + $cap.Snap + '，图标菜单=' + $cap.IconMenu)
    if ($cap.Focused) {
        $snap_skip = '本机 Shell 不提供贴靠浮窗（对照程序 regedit 同样不出）'
        $menu_skip = '本机 Shell 不提供标题栏图标菜单（对照程序 regedit 同样不出）'
    } else {
        $snap_skip = '对照未取得前台，按不具备处理'
        $menu_skip = '对照未取得前台，按不具备处理'
    }

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
    # 双击拖区中间，避开最左的图标命中区。
    Double-Click ([int](($rect.Left + $rect.Right) / 2)) ($rect.Top + 16)
    $max = Rect $app.MainWindowHandle
    $wide = ($max.Right - $max.Left) -ge ([Probe.U]::GetSystemMetrics(0) - 40)
    Check '双击头部能最大化' $wide ('宽 ' + ($max.Right - $max.Left) + '，屏幕 ' + [Probe.U]::GetSystemMetrics(0))
    Stop-App $app

    Write-Host '贴靠布局：'
    $app = Start-App
    $style = [uint32](([int64][Probe.U]::GetWindowLongPtrW($app.MainWindowHandle, -16)) -band 0xFFFFFFFF)
    $caption_style = (($style -band 0x00080000) -ne 0) -and (($style -band 0x00010000) -ne 0) -and (($style -band 0x00020000) -ne 0)
    Check '窗口样式含系统菜单与最大/最小化框' $caption_style ('GWL_STYLE=0x' + $style.ToString('X8'))
    $zones = Find-CaptionZones $app.MainWindowHandle
    $max_zone = $zones[9]
    Check '最大化按钮交给系统' ($null -ne $max_zone) '标题栏顶部没找到 HTMAXBUTTON 命中区'
    if ($null -ne $max_zone) {
        if ($cap.Snap) {
            Move-To 8 860
            Start-Sleep -Milliseconds 300
            $before = Snapshot-Windows
            [void][Probe.U]::SetCursorPos($max_zone.X, $max_zone.Y)
            Start-Sleep -Milliseconds 150
            [void][Probe.U]::SetCursorPos($max_zone.X + 1, $max_zone.Y)
            Start-Sleep -Milliseconds 150
            [void][Probe.U]::SetCursorPos($max_zone.X, $max_zone.Y)
            Start-Sleep -Milliseconds 900
            $during = Snapshot-Windows
            $appeared = @($during.Keys | Where-Object { -not $before.ContainsKey($_) })
            $classes = (@($appeared | ForEach-Object { $during[$_] })) -join ','
            Check '悬停最大化按钮弹出贴靠布局' ($appeared.Count -gt 0) ('悬停前后顶层窗口相同，类名：' + $classes)
            [void][Probe.U]::SetCursorPos(8, 860)
            Start-Sleep -Milliseconds 900
            $after = Snapshot-Windows
            $left = @($appeared | Where-Object { $after.ContainsKey($_) })
            Check '指针移开后贴靠布局消失' ($left.Count -eq 0) ('还留着 ' + $left.Count + ' 个窗口')
        } else {
            Skip '悬停最大化按钮弹出贴靠布局' $snap_skip
            Skip '指针移开后贴靠布局消失' $snap_skip
        }
    }
    Stop-App $app

    Write-Host '系统菜单：'
    $app = Start-App
    $zones = Find-CaptionZones $app.MainWindowHandle
    $icon_zone = $zones[3]
    if ($null -eq $icon_zone) {
        Check '单击图标弹出系统菜单' $false '标题栏顶部没找到 HTSYSMENU 命中区'
    } elseif (-not $cap.IconMenu) {
        Skip '单击图标弹出系统菜单' $menu_skip
    } else {
        Click $icon_zone.X $icon_zone.Y
        Start-Sleep -Milliseconds 500
        $menu = [Probe.U]::FindWindow('#32768', $null)
        $menu_pid = 0
        if ($menu -ne [IntPtr]::Zero) { [void][Probe.U]::GetWindowThreadProcessId($menu, [ref]$menu_pid) }
        Check '单击图标弹出系统菜单' (($menu -ne [IntPtr]::Zero) -and ($menu_pid -eq $app.Id)) ('菜单句柄 ' + $menu + '，进程 ' + $menu_pid)
        if ($menu -ne [IntPtr]::Zero) {
            [Probe.U]::keybd_event([byte]0x1B, [byte]0, 0, [IntPtr]::Zero)
            [Probe.U]::keybd_event([byte]0x1B, [byte]0, 0x0002, [IntPtr]::Zero)
            Start-Sleep -Milliseconds 400
            $gone = [Probe.U]::FindWindow('#32768', $null)
            Check 'Esc 收掉系统菜单' ($gone -eq [IntPtr]::Zero) ('菜单还在：' + $gone)
        }
    }
    Stop-App $app

    Write-Host '双击图标关窗：'
    $app = Start-App
    $zones = Find-CaptionZones $app.MainWindowHandle
    $icon_zone = $zones[3]
    if ($null -eq $icon_zone) {
        Check '双击图标关窗' $false '标题栏顶部没找到 HTSYSMENU 命中区'
        Stop-App $app
    } else {
        Double-Click $icon_zone.X $icon_zone.Y
        $exited = $app.WaitForExit(3000)
        $alive = Get-Process -Id $app.Id -ErrorAction SilentlyContinue
        Check '双击图标关窗' ($exited -and ($null -eq $alive)) ('退出=' + $exited + '，进程还在=' + ($null -ne $alive))
        if ($null -ne $alive) { Stop-App $app }
    }
} finally {
    Pop-Location
}

if ($failures -gt 0) {
    Write-Host ''
    Write-Host ('跳过 ' + $skipped + ' 条（本机 Shell 不提供对应行为）')
    Write-Host ($failures.ToString() + ' 条失败')
    exit 1
}
Write-Host ''
Write-Host ('跳过 ' + $skipped + ' 条（本机 Shell 不提供对应行为）')
Write-Host '全部通过'
exit 0
