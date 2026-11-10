# Real installed WebView + authored npm/Codex processes, NOT official agent runtime.
# Requires disposable hosted runner guard and the existing UIAutomation helpers.
function Get-InstallAccessibleText {
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    if ($null -eq $window) { throw 'Installed application window missing' }
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    return (@($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ }) -join "`n")
}
function Wait-InstallText([string]$expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if ((Get-InstallAccessibleText).Contains($expected)) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Expected synthetic install UI evidence missing'
}
function Assert-InstallNoConfirmation {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ($null -eq (Find-QwenControl '确认安装软件' ([Windows.Automation.ControlType]::Button))) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Install confirmation was not cleared'
}
function Assert-InstallNoConfiguration {
    foreach ($path in @('.codex', '.agentdeck/profiles', '.agentdeck/vault', '.agentdeck/backups')) {
        if (Test-Path -LiteralPath (Join-Path $homeDir $path)) { throw 'Synthetic installation created configuration state' }
    }
    Assert-Sentinel
}
function Invoke-InstallFixture([string[]]$arguments) {
    $savedConsoleEncoding = [Console]::OutputEncoding
    $savedOutputEncoding = $OutputEncoding
    try {
        # Python emits UTF-8 even when the Windows locale is cp1252; PowerShell
        # must decode its native stdout with that same explicit encoding.
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $OutputEncoding = [Text.UTF8Encoding]::new($false)
        $output = & $script:acceptancePython -X utf8 (Join-Path $PSScriptRoot 'windows_install_fixture.py') @arguments
        if ($LASTEXITCODE -ne 0) { throw 'Synthetic installation fixture guard failed' }
        return $output
    } finally {
        [Console]::OutputEncoding = $savedConsoleEncoding
        $OutputEncoding = $savedOutputEncoding
    }
}
function Get-InstallPlanNames {
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    $plans = $window.FindAll([Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, 'OpenAI Codex 安装计划'))
    if ($plans.Count -ne 1) { throw 'Missing or ambiguous installation plan' }
    $nodes = $plans[0].FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    return @($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
}
function Open-ReviewedInstallPlan($fixture, [string]$manifest) {
    $beforeQueries = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
    Click-Qwen '安装 OpenAI Codex'
    $null = Wait-QwenControl '确认安装软件' ([Windows.Automation.ControlType]::Button)
    $names = Get-InstallPlanNames
    foreach ($value in @('@openai/codex', 'codex', $fixture.prefix, $fixture.globalRoot, $fixture.package,
                         $fixture.manager, $fixture.node)) {
        if ($names -cnotcontains $value) { throw 'Reviewed installation path/package mismatch' }
    }
    $text = $names -join "`n"
    foreach ($policy in @('生命周期脚本：禁用（ignore-scripts）', '仅安装软件，不应用 profile', '版本由当前 npm 配置选择')) {
        if (-not $text.Contains($policy)) { throw 'Reviewed installation policy mismatch' }
    }
    $afterQueries = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
    if ($afterQueries.prefixQueries -ne ($beforeQueries.prefixQueries + 1) -or $afterQueries.rootQueries -ne ($beforeQueries.rootQueries + 1)) { throw 'Preview query count mismatch' }
    Assert-InstallNoConfiguration
}
function Check-ReviewedInstall {
    $savedPath = $env:PATH; $savedExt = $env:PATHEXT
    $fixtureRoot = Join-Path $root 'Windows authored install fixture'
    $manifest = Invoke-InstallFixture @('--prepare', $fixtureRoot, '--node', $script:acceptanceNode, '--home', $homeDir)
    $fixture = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
    try {
        $env:PATH = "$($fixture.path);$savedPath"
        $env:PATHEXT = '.COM;.EXE;.BAT;.CMD'
        Check-Gui 'reviewed-install-start' -KeepOpen
        # Scope the directory to one card. Native ValuePattern, never DOM/IPC.
        Set-QwenPath '搜索 Agent' 'Codex'
        $null = Wait-QwenControl '安装 OpenAI Codex' ([Windows.Automation.ControlType]::Button)
        Assert-InstallNoConfiguration
        $baseline = Get-CodexSnapshot
        Open-ReviewedInstallPlan $fixture $manifest
        $null = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest)
        Click-Qwen '取消'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ((Get-CodexSnapshot) -cne $baseline) { throw 'Cancelled installation changed configuration' }
        Save-QwenEvidence 'reviewed-install-cancel'
        Record 'reviewed-install-cancel' @{ fixtureCounts = $counts; configurationUnchanged = $true }

        Open-ReviewedInstallPlan $fixture $manifest
        $null = Invoke-InstallFixture @('--stale', '--manifest', $manifest)
        Click-Qwen '确认安装软件'
        Wait-InstallText 'Installation target changed since confirmation'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ((Get-CodexSnapshot) -cne $baseline) { throw 'Stale installation changed configuration' }
        Save-QwenEvidence 'reviewed-install-stale'
        Record 'reviewed-install-stale' @{ fixtureCounts = $counts; configurationUnchanged = $true }

        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '确认安装软件'
        Wait-InstallText '软件安装操作已完成，未应用配置。'
        Wait-InstallText 'v1.2.3'
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        while ((Get-InstallAccessibleText).Contains('安装操作处理中')) {
            if ([DateTime]::UtcNow -ge $deadline) { throw 'Installed card did not settle' }
            Start-Sleep -Milliseconds 100
        }
        $counts = Invoke-InstallFixture @('--assert', 'installed', '--manifest', $manifest) | ConvertFrom-Json
        # Invoke the exact detail control, not a reflow-sensitive coordinate.
        Click-Qwen '查看 OpenAI Codex 详情'
        Wait-InstallText '实测版本 1.2.3'
        Wait-InstallText 'npm 全局目录匹配，安装历史未知'
        foreach ($value in @('@openai/codex', $fixture.package, $fixture.globalRoot, $fixture.prefix, (Join-Path $fixture.managerBin 'npm.cmd'))) {
            Wait-InstallText $value
        }
        $null = Invoke-InstallFixture @('--assert', 'no-config', '--manifest', $manifest)
        Assert-InstallNoConfiguration
        if ((Get-CodexSnapshot) -cne $baseline) { throw 'Installation changed configuration' }
        Save-QwenEvidence 'reviewed-install-source'
        Record 'reviewed-install-complete' @{ fixtureCounts = $counts; stages = @('preview_cancel','stale_revision','confirmed_install','observed_source','no_configuration'); authoredNpmAndAgentCodeExecuted = $true; officialAgentExecuted = $false; realRegistryCompatibilityVerified = $false }
    } finally {
        Stop-App
        $env:PATH = $savedPath; $env:PATHEXT = $savedExt
    }
}
