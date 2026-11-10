# Installed Windows WebView observation only. No direct IPC, JS or Codex runtime.
# Dot-source only after the disposable hosted-runner guard and Qwen UI helpers.
function Get-CodexSnapshot {
    $items = @()
    $missing = @()
    foreach ($name in @('.codex', '.agentdeck')) {
        $base = Join-Path $homeDir $name
        if (-not (Test-Path -LiteralPath $base)) { $missing += $name; continue }
        $items += @((Get-Item -LiteralPath $base -Force)) + @(Get-ChildItem -LiteralPath $base -Force -Recurse)
    }
    if ($items.Count -gt 256) { throw 'Codex fixture unexpectedly large' }
    $values = foreach ($item in ($items | Sort-Object FullName)) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Codex fixture reparse point' }
        $hash = if ($item.PSIsContainer) { '' } else { (Get-FileHash -LiteralPath $item.FullName).Hash }
        # Access times can change on read; do not confuse them with writes.
        [ordered]@{ path = $item.FullName; directory = $item.PSIsContainer; hash = $hash;
            length = if ($item.PSIsContainer) { 0 } else { $item.Length };
            attributes = [int]$item.Attributes; created = $item.CreationTimeUtc.Ticks;
            modified = $item.LastWriteTimeUtc.Ticks; acl = (Get-Acl -LiteralPath $item.FullName).Sddl }
    }
    return ConvertTo-Json -InputObject @{ missing = $missing; entries = @($values) } -Depth 5 -Compress
}
function Assert-CodexNoWriter {
    if ($null -ne (Find-QwenControl '启用终端动画' ([Windows.Automation.ControlType]::CheckBox))) {
        throw 'Windows readiness exposed unsupported write checkbox'
    }
    foreach ($name in @('确认应用动画变更','确认恢复动画配置','预览动画变更','检查动画恢复状态')) {
        if ($null -ne (Find-QwenControl $name ([Windows.Automation.ControlType]::Button))) {
            throw 'Windows readiness exposed an unsupported write/recovery control'
        }
    }
}
function Assert-CodexQuiet([string]$before) {
    if ((Get-CodexSnapshot) -cne $before) { throw 'Read-only Codex readiness changed configuration or metadata' }
    if (Test-Path -LiteralPath $script:codexProcessMarker) { throw 'Read-only readiness invoked synthetic Codex sentinel' }
    Assert-CodexNoWriter
}
function Observe-Codex([string]$expected, [string]$stage) {
    $before = Get-CodexSnapshot
    Click-Qwen '检查动画配置（只读）'
    Wait-QwenText $expected
    Assert-CodexQuiet $before
    Save-QwenEvidence $stage
    Record $stage @{ readOnly = $true; exactConfigurationAndMetadataUnchanged = $true; syntheticSentinelNotInvoked = $true; noWriterControls = $true }
}
function Clear-CodexObservation {
    $before = Get-CodexSnapshot
    Click-Qwen '取消确认 / 清除结果'
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ($null -eq (Find-QwenControl '取消确认 / 清除结果' ([Windows.Automation.ControlType]::Button))) {
            Assert-CodexQuiet $before
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Codex readiness result was not cleared'
}
function Check-CodexReadonly {
    $directory = Join-Path $homeDir '.codex'
    if (Test-Path -LiteralPath $directory) { throw 'Codex readiness requires absent initial fixture' }
    $bin = Join-Path $homeDir 'Codex observation sentinel'
    New-Item -ItemType Directory -Path $bin | Out-Null
    $script:codexProcessMarker = Join-Path $bin 'invoked.txt'
    $launcher = Join-Path $bin 'codex.cmd'
    $savedPath = $env:PATH
    $savedCodexHome = $env:CODEX_HOME
    try {
        # Launch with an empty search directory so initial discovery cannot invoke
        # the sentinel. Install it only once the native startup gate has completed.
        $env:PATH = "$bin;$savedPath"
        $env:CODEX_HOME = $directory
        Check-Gui 'codex-readiness-start' -KeepOpen
        [IO.File]::WriteAllText($launcher, "@echo off`r`necho invoked>`"$script:codexProcessMarker`"`r`nexit /b 64`r`n", [Text.Encoding]::ASCII)
        $launcherHash = (Get-FileHash -LiteralPath $launcher).Hash
        Observe-Codex '动画配置文件不存在；未创建文件。' 'codex-readiness-missing'
        Clear-CodexObservation
        if (Test-Path -LiteralPath $directory) { throw 'Missing observation created Codex directory' }
        New-Item -ItemType Directory -Path $directory | Out-Null
        $config = Join-Path $directory 'config.toml'
        $original = "# SYNTHETIC-NOT-A-CREDENTIAL`n[tui]`nanimations = true # preserve`n"
        [IO.File]::WriteAllText($config, $original, [Text.UTF8Encoding]::new($false))
        Observe-Codex '当前动画值：true。只读观察，不代表可应用或 Codex 已加载。' 'codex-readiness-true'
        Clear-CodexObservation
        [IO.File]::WriteAllText($config, $original.Replace('= true', '= false'), [Text.UTF8Encoding]::new($false))
        Observe-Codex '当前动画值：false。只读观察，不代表可应用或 Codex 已加载。' 'codex-readiness-external-false'
        Clear-CodexObservation
        [IO.File]::WriteAllText($config, "# SYNTHETIC-NOT-A-CREDENTIAL`n[tui`n", [Text.UTF8Encoding]::new($false))
        Observe-Codex '动画配置格式或路径不受支持；未修改文件。' 'codex-readiness-unsupported'
        Clear-CodexObservation
        [IO.File]::WriteAllText($config, $original, [Text.UTF8Encoding]::new($false))
        [IO.File]::WriteAllText((Join-Path $directory '.agentdeck-codex-animations.json'), '{"fixture":"SYNTHETIC-NOT-A-CREDENTIAL"}', [Text.UTF8Encoding]::new($false))
        Observe-Codex '检测到受管收据；未读取收据，Windows 暂不支持恢复。' 'codex-readiness-receipt'
        Clear-CodexObservation
        if ((Get-FileHash -LiteralPath $launcher).Hash -ne $launcherHash) { throw 'Codex sentinel changed' }
        Record 'codex-readiness-complete' @{ stages = 5; actualCodexExecutionVerified = $false; windowsWritingSupported = $false }
    } finally {
        Stop-App
        $env:PATH = $savedPath
        $env:CODEX_HOME = $savedCodexHome
        if (Test-Path -LiteralPath $launcher) { Remove-Item -LiteralPath $launcher }
    }
}
