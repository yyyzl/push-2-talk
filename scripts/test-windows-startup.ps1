<#
验证真实 Windows 程序在连续启动时能完成初始化，捕获窗口先于状态注册的竞态。
请在隔离测试账号或已备份、隔离配置的环境中运行；脚本不安装程序、不覆盖配置。
每轮结束仅终止本脚本启动的进程，日志保留在指定目录；整组测试最多 55 秒。
#>
param(
    [Parameter(Mandatory)][string]$ExecutablePath,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(1, 20)][int]$RunCount = 10,
    [ValidateRange(1, 10)][int]$StartupTimeoutSeconds = 6
)

$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $ExecutablePath).Path
if ([IO.Path]::GetExtension($exe) -ne '.exe') { throw '必须指定 Windows 可执行程序' }
$processName = [IO.Path]::GetFileNameWithoutExtension($exe)
if (Get-Process -Name $processName -ErrorAction SilentlyContinue) {
    throw '被测程序已在运行，请先退出，以免单实例转发造成假通过'
}
$logDirectory = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $logDirectory) { throw '输出目录已存在，请使用新的目录以保留历史证据' }
New-Item -ItemType Directory -Path $logDirectory | Out-Null
$oldRustLog = $env:RUST_LOG
$results = @()
$total = [Diagnostics.Stopwatch]::StartNew()
try {
    $env:RUST_LOG = 'info'
    for ($index = 1; $index -le $RunCount; $index++) {
        if ($total.Elapsed.TotalSeconds -ge 55) { throw '启动测试达到总超时限制' }
        $stdout = Join-Path $logDirectory "startup-$index.log"
        $stderr = Join-Path $logDirectory "startup-$index-stderr.log"
        $child = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        $timer = [Diagnostics.Stopwatch]::StartNew()
        $ready = $false
        $panicked = $false
        try {
            do {
                Start-Sleep -Milliseconds 100
                $outText = [string](Get-Content -LiteralPath $stdout -Raw -Encoding utf8)
                $errText = [string](Get-Content -LiteralPath $stderr -Raw -Encoding utf8)
                $ready = [bool]($outText -match '启动完成!')
                $panicked = [bool]($errText -match 'panicked')
                $child.Refresh()
            } while (-not $ready -and -not $panicked -and -not $child.HasExited `
                -and $timer.Elapsed.TotalSeconds -lt $StartupTimeoutSeconds `
                -and $total.Elapsed.TotalSeconds -lt 55)

            if ($ready) {
                # 就绪后再观察一次，避免漏掉同一启动阶段稍晚发生的 panic。
                Start-Sleep -Milliseconds 500
                $errText = [string](Get-Content -LiteralPath $stderr -Raw -Encoding utf8)
                $panicked = [bool]($errText -match 'panicked')
                $child.Refresh()
            }
            $results += [pscustomobject]@{
                Run = $index
                Started = $ready
                Panicked = $panicked
                StateRegistrationPanic = [bool]($errText -match 'state\(\) called before manage\(\)')
                ExitedBeforeReady = $child.HasExited
                ElapsedMs = $timer.ElapsedMilliseconds
            }
            $results[-1] | ConvertTo-Json -Compress | Write-Output
        } finally {
            $child.Refresh()
            if (-not $child.HasExited) {
                Stop-Process -Id $child.Id
                $null = $child.WaitForExit(2000)
            }
            $child.Dispose()
        }
        Start-Sleep -Milliseconds 300
    }
} finally {
    $env:RUST_LOG = $oldRustLog
    $results | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $logDirectory 'results.json') -Encoding utf8
}
if ($results.Count -ne $RunCount -or ($results | Where-Object { -not $_.Started -or $_.Panicked -or $_.ExitedBeforeReady })) {
    throw '启动回归失败，详情见输出目录中的日志与 results.json'
}
