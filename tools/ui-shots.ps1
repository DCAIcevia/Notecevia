# 界面截图与逐像素比对（本地验收，不进 CI：需要真窗口）
#
# 用法：
#   pwsh tools/ui-shots.ps1 -Capture before          # 改之前截一组
#   pwsh tools/ui-shots.ps1 -Capture after           # 改之后截一组
#   pwsh tools/ui-shots.ps1 -Before before -After after   # 逐像素比，有差异就非零退出
#
# 图都落在 target/ui-shots/ 下，不会进仓库。每个状态用一个新工作区，互不污染。

param(
    [string]$Capture,
    [string]$Before,
    [string]$After
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $exe = Join-Path $root 'target\debug\notecevia.exe'
    if (-not (Test-Path $exe)) { throw '先 cargo build -p notecevia-app' }

    $base = Join-Path $root 'target\ui-shots'

    if ($Before -or $After) {
        if (-not ($Before -and $After)) { throw '用法：-Before <名字> -After <名字>' }
        & cargo run -q -p notecevia-app --example shot_diff -- (Join-Path $base $Before) (Join-Path $base $After)
        exit $LASTEXITCODE
    }
    if (-not $Capture) { throw '用法：-Capture <名字> 或 -Before <名字> -After <名字>' }

    # 固定的几屏：首页、画布、压力画布、册、回收站、窄窗放大。
    $cases = @(
        @{ name = 'home';         flags = @('--home') },
        @{ name = 'board';        flags = @() },
        @{ name = 'board-stress'; flags = @('--stress') },
        @{ name = 'volume';       flags = @('--volume') },
        @{ name = 'trash';        flags = @('--trash') },
        @{ name = 'home-tight';   flags = @('--home', '--size', '900x520', '--zoom', '1.5') }
    )

    $out = Join-Path $base $Capture
    New-Item -ItemType Directory -Path $out -Force | Out-Null

    $work = Join-Path ([System.IO.Path]::GetTempPath()) ('notecevia-shots-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $seed = Join-Path $work 'seed'
        & cargo run -q -p notecevia-store --example make_fixture -- $seed | Out-Null
        foreach ($case in $cases) {
            $workspace = Join-Path $work $case.name
            Copy-Item $seed $workspace -Recurse
            $env:NOTECEVIA_HOME = $workspace
            $shot = Join-Path $out ($case.name + '.png')
            & $exe @($case.flags) --shot $shot | Out-Null
            if (-not (Test-Path $shot)) { throw ($case.name + ' 没截出来') }
        }
    } finally {
        Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    }
    Write-Host ('截好一组：' + $out)
} finally {
    Pop-Location
}
