# Real installed WebView + authored npm/Codex processes, NOT official agent runtime.
# Requires disposable hosted runner guard and the existing UIAutomation helpers.
function Get-InstallAccessibleText {
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    if ($null -eq $window) { throw 'Installed application window missing' }
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    return (@($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ }) -join "`n")
}
function Wait-InstallText([string]$expected,
    [ValidateSet('occupied-preview-error','authored-install-error','stale-error','install-complete','installed-version','version','source','package','global','prefix','manager')][string]$check) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if ((Get-InstallAccessibleText).Contains($expected)) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    Save-QwenEvidence ('reviewed-install-timeout-' + $check)
    Record 'reviewed-install-text-timeout' @{ check = $check }
    throw ('Expected synthetic install UI evidence missing: ' + $check)
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
function Open-InstalledCodexDetails {
    # aria-expanded buttons may expose ExpandCollapse rather than Invoke in
    # WebView2. Choose one supported native action before activating; never
    # retry another action after an uncertain expand/key delivery.
    if ($null -ne (Find-QwenControl '收起 OpenAI Codex 详情' ([Windows.Automation.ControlType]::Button))) {
        throw 'Installed Codex details unexpectedly already expanded'
    }
    $control = Wait-QwenControl '查看 OpenAI Codex 详情' ([Windows.Automation.ControlType]::Button)
    $pattern = $null
    if ($control.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) {
        $expand = [Windows.Automation.ExpandCollapsePattern]$pattern
        if ($expand.Current.ExpandCollapseState -ne [Windows.Automation.ExpandCollapseState]::Collapsed) {
            throw 'Installed Codex details initial state is not collapsed'
        }
        Record 'reviewed-install-details-activation' @{ control = 'codex-details'; pattern = 'ExpandCollapse'; initialState = 'collapsed' }
        $expand.Expand()
    } else {
        Add-Type -AssemblyName System.Windows.Forms
        $control.SetFocus()
        $deadline = [DateTime]::UtcNow.AddSeconds(5)
        while (-not $control.Current.HasKeyboardFocus) {
            if ([DateTime]::UtcNow -ge $deadline) { throw 'Installed Codex details focus not confirmed' }
            Start-Sleep -Milliseconds 100
        }
        Record 'reviewed-install-details-activation' @{ control = 'codex-details'; pattern = 'focused-native-enter'; initialState = 'collapsed' }
        [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    }
    $null = Wait-QwenControl '收起 OpenAI Codex 详情' ([Windows.Automation.ControlType]::Button)
    $null = Wait-QwenControl '关闭 OpenAI Codex 详情' ([Windows.Automation.ControlType]::Button)
    if ($null -ne (Find-QwenControl '查看 OpenAI Codex 详情' ([Windows.Automation.ControlType]::Button))) {
        throw 'Installed Codex details did not remain expanded'
    }
}
function Check-ReviewedInstall {
    $savedPath = $env:PATH; $savedExt = $env:PATHEXT
    $fixtureRoot = Join-Path $root 'Windows authored install fixture'
    $manifest = Invoke-InstallFixture @('--prepare', $fixtureRoot, '--node', $script:acceptanceNode, '--home', $homeDir)
    $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
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
        Wait-InstallText 'Installation target changed since confirmation' 'stale-error'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ((Get-CodexSnapshot) -cne $baseline) { throw 'Stale installation changed configuration' }
        Save-QwenEvidence 'reviewed-install-stale'
        Record 'reviewed-install-stale' @{ fixtureCounts = $counts; configurationUnchanged = $true }

        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '确认安装软件'
        Wait-InstallText '软件安装操作已完成，未应用配置。' 'install-complete'
        Wait-InstallText 'v1.2.3' 'installed-version'
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        while ((Get-InstallAccessibleText).Contains('安装操作处理中')) {
            if ([DateTime]::UtcNow -ge $deadline) { throw 'Installed card did not settle' }
            Start-Sleep -Milliseconds 100
        }
        $counts = Invoke-InstallFixture @('--assert', 'installed', '--manifest', $manifest) | ConvertFrom-Json
        Record 'reviewed-install-confirmed' @{ fixtureCounts = $counts; authoredNpmAndAgentCodeExecuted = $true; officialAgentExecuted = $false }
        # Invoke the exact detail control, not a reflow-sensitive coordinate.
        Open-InstalledCodexDetails
        Save-QwenEvidence 'reviewed-install-details-expanded'
        Wait-InstallText '实测版本 1.2.3' 'version'
        Wait-InstallText 'npm 全局目录匹配，安装历史未知' 'source'
        Wait-InstallText '@openai/codex' 'package'
        Wait-InstallText $fixture.package 'package'
        Wait-InstallText $fixture.globalRoot 'global'
        Wait-InstallText $fixture.prefix 'prefix'
        Wait-InstallText $fixture.manager 'manager'
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
function Get-FailedInstallState($fixture) {
    # Exact residual state, not mutable query logs. Never write these private paths
    # or contents into public evidence; retain only the comparison result.
    $items = @((Get-Item -LiteralPath $fixture.prefix -Force), (Get-Item -LiteralPath $fixture.marker -Force))
    $items += @(Get-ChildItem -LiteralPath $fixture.prefix -Recurse -Force)
    if ($items.Count -gt 32) { throw 'Unexpected failed installation residual size' }
    $values = foreach ($item in ($items | Sort-Object FullName)) {
        # Directory enumeration caches metadata. Query the same entry again so
        # a pre-existing cached parent mtime is not compared with a later live
        # query. Keep all timestamp/attribute/ACL/byte assertions intact.
        $item.Refresh()
        if (-not $item.Exists) { throw 'Failed installation residual disappeared during snapshot' }
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Failed installation residual became a reparse point' }
        [ordered]@{ path = $item.FullName; directory = $item.PSIsContainer;
            hash = if ($item.PSIsContainer) { '' } else { (Get-FileHash -LiteralPath $item.FullName).Hash };
            length = if ($item.PSIsContainer) { 0 } else { $item.Length };
            created = $item.CreationTimeUtc.Ticks; modified = $item.LastWriteTimeUtc.Ticks;
            attributes = [int]$item.Attributes; acl = (Get-Acl -LiteralPath $item.FullName).Sddl }
    }
    return ConvertTo-Json -InputObject @($values) -Depth 5 -Compress
}
function Get-FailedStateDifference([string]$before, [string]$after) {
    # Compare privately, emit only fixed field names and numeric entry indices.
    # Never emit paths, hashes, ACL values, file contents or timestamp values.
    $old = @($before | ConvertFrom-Json)
    $new = @($after | ConvertFrom-Json)
    $changes = @()
    if ($old.Count -ne $new.Count) { $changes += @{ entryIndex = -1; fields = @('entry-count') } }
    for ($index = 0; $index -lt [Math]::Min($old.Count, $new.Count); $index++) {
        $fields = @()
        foreach ($field in @('path','directory','hash','length','created','modified','attributes','acl')) {
            if ($old[$index].$field -cne $new[$index].$field) { $fields += $field }
        }
        if ($fields.Count -gt 0) { $changes += @{ entryIndex = $index; fields = $fields } }
    }
    if ($changes.Count -eq 0 -and $before -cne $after) { $changes += @{ entryIndex = -1; fields = @('serialization-only') } }
    return ,$changes
}
function Assert-FailedInstallObservation($fixture, [string]$manifest, [string]$baseline, [string]$residual,
    [ValidateSet('initial','settled','refresh','restart','occupied-preview')][string]$stage) {
    $counts = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest) | ConvertFrom-Json
    if ($counts.installCalls -ne 1 -or $counts.versionProbes -ne 0 -or $counts.partialInstallRetained -ne $true) {
        throw 'Failed installation was retried, executed an agent or lost partial state'
    }
    $observedResidual = Get-FailedInstallState $fixture
    if ($observedResidual -cne $residual) {
        $difference = Get-FailedStateDifference $residual $observedResidual
        Record 'failed-reviewed-install-residual-difference' @{ checkpoint = $stage; changes = $difference }
        throw ('Failed installation residual changed: ' + $stage)
    }
    if ((Get-CodexSnapshot) -cne $baseline) { throw 'Failed installation changed configuration' }
    Assert-InstallNoConfiguration
    Assert-InstallNoConfirmation
    if ((Get-InstallAccessibleText).Contains('软件安装操作已完成')) { throw 'Failed installation displayed success' }
    return $counts
}
function Check-FailedReviewedInstall {
    $savedPath = $env:PATH; $savedExt = $env:PATHEXT
    $fixtureRoot = Join-Path $root 'Windows authored failed install fixture'
    $manifest = Invoke-InstallFixture @('--prepare', $fixtureRoot, '--node', $script:acceptanceNode, '--home', $homeDir, '--mode', 'fail')
    $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
    try {
        $env:PATH = "$($fixture.path);$savedPath"
        $env:PATHEXT = '.COM;.EXE;.BAT;.CMD'
        Check-Gui 'failed-reviewed-install-start' -KeepOpen
        Set-QwenPath '搜索 Agent' 'Codex'
        $null = Wait-QwenControl '安装 OpenAI Codex' ([Windows.Automation.ControlType]::Button)
        Assert-InstallNoConfiguration
        $baseline = Get-CodexSnapshot
        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '取消'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ($counts.installCalls -ne 0 -or $counts.versionProbes -ne 0) { throw 'Cancelled failure fixture executed installation' }
        if ((Get-CodexSnapshot) -cne $baseline) { throw 'Cancelled failure fixture changed configuration' }
        Save-QwenEvidence 'failed-reviewed-install-cancel'
        Record 'failed-reviewed-install-cancel' @{ fixtureCounts = $counts; configurationUnchanged = $true }

        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '确认安装软件'
        Wait-InstallText '安装失败（软件可能已安装，请查看检测状态）' 'authored-install-error'
        Assert-InstallNoConfirmation
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        $null = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest)
        $residual = Get-FailedInstallState $fixture
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'initial'
        # Observe the settled failure for a bounded interval, without retrying any
        # action. The event log counts attempts before exclusive-marker rejection.
        Start-Sleep -Milliseconds 1000
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'settled'
        Save-QwenEvidence 'failed-reviewed-install-confirmed'
        Record 'failed-reviewed-install-confirmed' @{ fixtureCounts = $counts; residualUnchanged = $true; noSuccessNotice = $true }

        Click-Qwen '刷新列表'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Start-Sleep -Milliseconds 1000
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'refresh'
        Save-QwenEvidence 'failed-reviewed-install-refreshed'
        Record 'failed-reviewed-install-refreshed' @{ fixtureCounts = $counts; residualUnchanged = $true; configurationUnchanged = $true }

        Stop-App
        Check-Gui 'failed-reviewed-install-restart' -KeepOpen
        Set-QwenPath '搜索 Agent' 'Codex'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'restart'
        Save-QwenEvidence 'failed-reviewed-install-restarted'
        # A partial package occupies the selected destination. A fresh preview
        # must be rejected by production guards; never erase it to force a retry.
        Click-Qwen '安装 OpenAI Codex'
        Wait-InstallText '无法取得有效安装预览，请刷新后重新检查。' 'occupied-preview-error'
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'occupied-preview'
        Save-QwenEvidence 'failed-reviewed-install-occupied-preview'
        Record 'failed-reviewed-install-occupied-preview' @{ fixtureCounts = $counts; confirmationAbsent = $true; residualUnchanged = $true }
        Record 'failed-reviewed-install-complete' @{ fixtureCounts = $counts; stages = @('preview_cancel','confirmed_failure','refresh_no_retry','restart_no_retry','occupied_preview_rejected'); residualUnchanged = $true; configurationUnchanged = $true; authoredNpmExecuted = $true; authoredAgentExecuted = $false; officialAgentExecuted = $false; automaticRollbackObserved = $false; retryAttempted = $false }
    } finally {
        Stop-App
        $env:PATH = $savedPath; $env:PATHEXT = $savedExt
    }
}
