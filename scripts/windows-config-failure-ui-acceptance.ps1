# Installed Windows WebView only. Dot-source after hosted-runner guard/UI helpers.
# Synthetic version-only packages, no installation, credentials or official agent.
function Invoke-ConfigurationFixture([string[]]$arguments) {
    $savedConsole = [Console]::OutputEncoding
    $savedOutput = $OutputEncoding
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $OutputEncoding = [Text.UTF8Encoding]::new($false)
        $output = & $script:acceptancePython -X utf8 (Join-Path $PSScriptRoot 'windows_config_failure_fixture.py') @arguments
        if ($LASTEXITCODE -ne 0) { throw 'Configuration fixture validation failed' }
        return $output
    } finally { [Console]::OutputEncoding = $savedConsole; $OutputEncoding = $savedOutput }
}
function Get-ConfigurationSnapshot($fixture, [string]$manifest) {
    # Python's Windows stat uses the volume/file identifier, not just equal bytes.
    $files = Invoke-ConfigurationFixture @('snapshot', '--manifest', $manifest) | ConvertFrom-Json
    $acls = @($fixture.configurationTargets | ForEach-Object { (Get-Acl -LiteralPath $_).Sddl })
    return ConvertTo-Json -InputObject @{ files = $files; acls = $acls } -Depth 5 -Compress
}
function Confirm-ConfigurationOnce([string]$name) {
    $expected = "确定为 $name 应用默认 profile 吗？将更新个性化指令与支持的模型配置，写入前会备份相关文件。"
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $processCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
        $windows = [Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children, $processCondition)
        $buttons = @()
        foreach ($window in $windows) {
            $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
            Assert-AcceptanceCanariesAbsent $nodes
            $names = @($nodes | ForEach-Object { $_.Current.Name })
            if ($names -cnotcontains $expected) { continue }
            $buttons += @($nodes | Where-Object {
                $_.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and
                $_.Current.Name -in @('OK','确定') -and $_.Current.IsEnabled
            })
        }
        if ($buttons.Count -gt 1) { throw 'Ambiguous native configuration confirmation' }
        if ($buttons.Count -eq 1) {
            # Choose one native action. Never retry after possible delivery.
            ([Windows.Automation.InvokePattern]$buttons[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Exact native configuration confirmation missing; no confirmation sent'
}
function Wait-ConfigurationMarkerError {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $text = Get-InstallAccessibleText
        if ($text.Contains('配置失败（可能已写入部分配置，请检查备份与当前配置）:') -and
            $text.Contains('Instruction file has ambiguous or incomplete AgentDeck markers; left unchanged')) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Expected malformed managed-marker error not observed through installed UI'
}
function Wait-ConfigurationLinkedTargetError {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $text = Get-InstallAccessibleText
        if ($text.Contains('配置失败（可能已写入部分配置，请检查备份与当前配置）:') -and
            $text.Contains('Configuration target or parent is linked or is not a regular file/directory; AgentDeck refused this target.') -and
            $text.Contains('Manage it with the tool that created the link') -and
            $text.Contains('No links were automatically detached.')) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Expected linked-target refusal not observed through installed UI'
}
function Check-ConfigurationHardlinkPreservation($manifest) {
    Stop-App
    $null = Invoke-ConfigurationFixture @('prepare-hardlinks', '--manifest', $manifest)
    $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($fixture.hardlinkAliases.Count -ne 2 -or $fixture.configurationTargets.Count -ne 6) { throw 'Incomplete hardlink fixture' }
    $baseline = Get-ConfigurationSnapshot $fixture $manifest
    $entries = @()
    foreach ($agent in @(@{ id = 'claude'; name = 'Claude Code' }, @{ id = 'gemini'; name = 'Gemini CLI' })) {
        Check-Gui ("configuration-hardlink-" + $agent.id + '-start') -KeepOpen
        Set-QwenPath '搜索 Agent' $agent.name
        $null = Wait-QwenControl ("应用默认配置 " + $agent.name) ([Windows.Automation.ControlType]::Button)
        if ((Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline) { throw 'Discovery changed hardlink fixture' }
        Click-Qwen ("应用默认配置 " + $agent.name)
        Confirm-ConfigurationOnce $agent.name
        Wait-ConfigurationLinkedTargetError
        $null = Wait-QwenControl ("应用默认配置 " + $agent.name) ([Windows.Automation.ControlType]::Button)
        if ((Get-InstallAccessibleText).Contains('默认 profile 配置操作已完成（写前备份）。')) { throw 'Linked configuration displayed success' }
        if ((Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline) { throw 'Linked configuration changed either alias or metadata' }
        $counts = Invoke-ConfigurationFixture @('verify', '--manifest', $manifest) | ConvertFrom-Json
        Save-QwenEvidence ("configuration-hardlink-" + $agent.id)
        Stop-App
        if ((Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline) { throw 'Hardlink fixture changed while closing application' }
        $entry = @{ agent = $agent.id; validContentHardlinkRefused = $true; confirmations = 1; bothAliasesBytesIdentityMetadataAclPreserved = $true; inactiveSettingsPreserved = $true; versionOnlyProbes = $counts; officialAgentExecuted = $false }
        $entries += $entry
        Record ("configuration-hardlink-" + $agent.id) $entry
    }
    if ($entries.Count -ne 2) { throw 'Missing hardlink rejection stage' }
    return $entries
}
function Check-ConfigurationFailurePreservation {
    Stop-App
    $savedPath = $env:PATH; $savedExt = $env:PATHEXT
    $fixtureRoot = Join-Path $root 'Windows authored configuration failure fixture'
    $manifest = Invoke-ConfigurationFixture @('prepare', '--root', $fixtureRoot, '--node', $script:acceptanceNode, '--home', $homeDir)
    $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
    $stage = 'configuration-failure-preservation'
    $result = @{ passed = $false; agents = @(); officialAgentExecuted = $false; installationAttempted = $false }
    try {
        # The production backup invokes real trusted Git and real Windows DPAPI.
        # Git's cmd directory contains no Agent executables. Do not restore broad PATH.
        $env:PATH = "$($fixture.path);$(Split-Path $script:acceptanceGit);$savedPath"
        $env:PATHEXT = '.COM;.EXE;.BAT;.CMD'
        $baseline = Get-ConfigurationSnapshot $fixture $manifest
        foreach ($agent in @(@{ id = 'claude'; name = 'Claude Code' }, @{ id = 'gemini'; name = 'Gemini CLI' })) {
            Check-Gui ("configuration-failure-" + $agent.id + '-start') -KeepOpen
            Set-QwenPath '搜索 Agent' $agent.name
            $null = Wait-QwenControl ("应用默认配置 " + $agent.name) ([Windows.Automation.ControlType]::Button)
            if ((Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline) { throw 'Discovery changed authored configuration' }
            Click-Qwen ("应用默认配置 " + $agent.name)
            Confirm-ConfigurationOnce $agent.name
            Wait-ConfigurationMarkerError
            $null = Wait-QwenControl ("应用默认配置 " + $agent.name) ([Windows.Automation.ControlType]::Button)
            if ((Get-InstallAccessibleText).Contains('默认 profile 配置操作已完成（写前备份）。')) { throw 'Rejected configuration displayed success' }
            # A specific adapter error proves the real git/DPAPI prerequisites passed.
            if (-not (Test-Path -LiteralPath (Join-Path $homeDir '.agentdeck/git-backup/.git'))) { throw 'Real configuration backup did not run' }
            if ((Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline) { throw 'Rejected configuration rewrote untouched files or metadata' }
            $counts = Invoke-ConfigurationFixture @('verify', '--manifest', $manifest) | ConvertFrom-Json
            Save-QwenEvidence ("configuration-failure-" + $agent.id)
            Stop-App
            if ((Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline) { throw 'Configuration changed while closing the application' }
            $entry = @{ agent = $agent.id; malformedMarkerError = $true; confirmations = 1; bytesAndFileIdentityAndMetadataPreserved = $true; inactiveModelSettingsPreserved = $true; versionOnlyProbes = $counts }
            $result.agents += $entry
            Record ("configuration-failure-" + $agent.id) $entry
        }
        if ($result.agents.Count -ne 2) { throw 'Missing configuration rejection stage' }
        $result.hardlinks = @(Check-ConfigurationHardlinkPreservation $manifest)
        $result.passed = $true
        Record $stage $result
    } finally {
        # Fixed booleans/counts only. No target paths, hashes, ACLs or authored text.
        $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "$stage.json") -Encoding utf8
        try { Stop-App } finally { $env:PATH = $savedPath; $env:PATHEXT = $savedExt }
    }
}
