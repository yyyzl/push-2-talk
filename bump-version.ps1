# 版本号一键更新脚本
# 用法: .\bump-version.ps1 1.5.3

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$Version
)

# 验证版本号格式
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Host "错误: 版本号格式不正确，应为 x.y.z 格式 (例如: 0.0.9)" -ForegroundColor Red
    exit 1
}

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path

$files = @(
    @{ Path = "$rootDir\package.json"; Pattern = '"version":\s*"[^"]*"'; Replace = "`"version`": `"$Version`"" },
    @{ Path = "$rootDir\src-tauri\Cargo.toml"; Pattern = '^version\s*=\s*"[^"]*"'; Replace = "version = `"$Version`"" },
    @{ Path = "$rootDir\src-tauri\tauri.conf.json"; Pattern = '"version":\s*"[^"]*"'; Replace = "`"version`": `"$Version`"" }
)

Write-Host "正在更新版本号为 $Version ..." -ForegroundColor Cyan

foreach ($file in $files) {
    if (Test-Path $file.Path) {
        # 读取内容 (Get-Content 通常能自动处理读取时的 BOM 问题)
        $content = Get-Content $file.Path -Raw -Encoding UTF8

        if ($file.Path -like "*Cargo.toml") {
            # Cargo.toml 只替换 [package] 下的第一个 version
            $lines = Get-Content $file.Path -Encoding UTF8
            $inPackage = $false
            $replaced = $false
            $newLines = @()

            foreach ($line in $lines) {
                if ($line -match '^\[package\]') {
                    $inPackage = $true
                }
                elseif ($line -match '^\[' -and $line -notmatch '^\[package\]') {
                    $inPackage = $false
                }

                if ($inPackage -and !$replaced -and $line -match '^version\s*=') {
                    $newLines += "version = `"$Version`""
                    $replaced = $true
                }
                else {
                    $newLines += $line
                }
            }

            # --- 修改部分开始 ---
            # 使用 .NET 类写入，明确禁止 BOM ([System.Text.UTF8Encoding]::new($false))
            [System.IO.File]::WriteAllLines($file.Path, $newLines, [System.Text.UTF8Encoding]::new($false))
            # --- 修改部分结束 ---
        }
        else {
            # JSON 文件只替换第一个匹配
            $newContent = $content -replace $file.Pattern, $file.Replace
            # 确保只替换了一次（找到第一个后停止）
            # 这里的逻辑本来就是正确的（无 BOM）
            [System.IO.File]::WriteAllText($file.Path, $newContent, [System.Text.UTF8Encoding]::new($false))
        }

        $relativePath = $file.Path.Replace($rootDir, "").TrimStart("\")
        Write-Host "  ✓ $relativePath" -ForegroundColor Green
    }
    else {
        $relativePath = $file.Path.Replace($rootDir, "").TrimStart("\")
        Write-Host "  ✗ $relativePath (文件不存在)" -ForegroundColor Red
    }
}

Write-Host "`n版本号已更新为 $Version" -ForegroundColor Green