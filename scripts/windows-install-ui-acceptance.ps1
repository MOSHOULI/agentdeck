# Real installed WebView + authored npm/Codex processes, NOT official agent runtime.
# Requires disposable hosted runner guard and the existing UIAutomation helpers.
function Get-InstallConfigurationSnapshot {
    # Generic Codex read-only snapshots still include every protection byte.
    # Installation itself legitimately transitions this one exact state subtree;
    # fixture assertions separately validate its inventory/schema and native
    # recovery assertions compare its bytes, identities and security descriptors.
    $snapshot = Get-CodexSnapshot | ConvertFrom-Json
    $protection = [IO.Path]::GetFullPath((Join-Path $homeDir '.agentdeck\install-protection'))
    $prefix = $protection.TrimEnd('\') + '\'
    $entries = @($snapshot.entries | Where-Object { $_.path -cne $protection -and -not $_.path.StartsWith($prefix,[StringComparison]::Ordinal) })
    return ConvertTo-Json -InputObject @{ missing = @($snapshot.missing); entries = $entries } -Depth 5 -Compress
}
function Get-InstallAccessibleText {
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    if ($null -eq $window) { throw 'Installed application window missing' }
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    return (@($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ }) -join "`n")
}
function Wait-InstallText([string]$expected,
    [ValidateSet('recovery-blocked','recovery-error','recovery-eligible','recovery-cleared','probe-suppression','session-boundary','residual-boundary','residual-unavailable','occupied-preview-error','authored-install-error','stale-error','install-complete','installed-version','version','source','package','global','prefix','manager')][string]$check) {
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
                         $fixture.manager, $fixture.node, $fixture.packageVersion, 'Codex 根包固定版本', $fixture.installerNodeVersion, $fixture.installerNodeMinimum,
                         '安装器 Node 实测版本', 'AgentDeck 安装器 Node 最低策略')) {
        if ($names -cnotcontains $value) { throw 'Reviewed installation path/package mismatch' }
    }
    $text = $names -join "`n"
    foreach ($policy in @('生命周期脚本：禁用（ignore-scripts）', '仅安装软件，不应用 profile', 'Codex 根包版本已固定')) {
        if (-not $text.Contains($policy)) { throw 'Reviewed installation policy mismatch' }
    }
    $afterQueries = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
    if ($afterQueries.metadataQueries -ne ($beforeQueries.metadataQueries + 1)) { throw 'Preview version metadata query count mismatch' }
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
        $baseline = Get-InstallConfigurationSnapshot
        Open-ReviewedInstallPlan $fixture $manifest
        $null = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest)
        Click-Qwen '取消'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ((Get-InstallConfigurationSnapshot) -cne $baseline) { throw 'Cancelled installation changed configuration' }
        Save-QwenEvidence 'reviewed-install-cancel'
        Record 'reviewed-install-cancel' @{ fixtureCounts = $counts; configurationUnchanged = $true }

        Open-ReviewedInstallPlan $fixture $manifest
        $null = Invoke-InstallFixture @('--stale', '--manifest', $manifest)
        Click-Qwen '确认安装软件'
        Wait-InstallText '安装请求已发送，但完成状态未确认。' 'stale-error'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ((Get-InstallConfigurationSnapshot) -cne $baseline) { throw 'Stale installation changed configuration' }
        $null = Invoke-InstallFixture @('--restore-latest', '--manifest', $manifest)
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
        if ((Get-InstallConfigurationSnapshot) -cne $baseline) { throw 'Installation changed configuration' }
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
    [ValidateSet('initial','settled','refresh','restart','recovery-blocked','session-observation','residual-inspection')][string]$stage) {
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
    if ((Get-InstallConfigurationSnapshot) -cne $baseline) { throw 'Failed installation changed configuration' }
    Assert-InstallNoConfiguration
    Assert-InstallNoConfirmation
    if ((Get-InstallAccessibleText).Contains('软件安装操作已完成')) { throw 'Failed installation displayed success' }
    return $counts
}
function Assert-InstallSessionAbsent {
    Assert-ResidualResultAbsent
    if ($null -ne (Find-QwenControl '只读检查固定残留槽位' ([Windows.Automation.ControlType]::Button))) { throw 'Residual inspection capability unexpectedly persisted across restart' }
    if ((Get-InstallAccessibleText).Contains('本会话安装记录：')) { throw 'Session installation record unexpectedly persisted across restart' }
}
function Observe-FailedInstallSession($fixture, [string]$manifest, [string]$baseline, [string]$residual) {
    $before = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest) | ConvertFrom-Json
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    $matches = $window.FindAll([Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, '本会话安装记录：安装完成状态未确认'))
    if ($matches.Count -ne 1) { throw 'Missing or ambiguous unconfirmed session record' }
    $control = $matches[0]
    $boundary = '这是本会话已确认请求的历史目标快照，不是当前磁盘检查、安装所有权或恢复凭据。关闭或重启应用后记录丢失。'
    if (-not (Get-InstallAccessibleText).Contains($boundary)) {
        $pattern = $null
        if ($control.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) {
            $expand = [Windows.Automation.ExpandCollapsePattern]$pattern
            if ($expand.Current.ExpandCollapseState -ne [Windows.Automation.ExpandCollapseState]::Collapsed) { throw 'Unexpected session record expand state' }
            $expand.Expand()
        } else {
            Add-Type -AssemblyName System.Windows.Forms
            $control.SetFocus()
            if (-not $control.Current.HasKeyboardFocus) { throw 'Session record focus not confirmed' }
            [Windows.Forms.SendKeys]::SendWait('{ENTER}')
        }
    }
    Wait-InstallText $boundary 'session-boundary'
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    Assert-AcceptanceCanariesAbsent $nodes
    $names = @($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
    foreach ($value in @('已审核 npm prefix','已审核全局目录','已审核包目录','已审核启动文件目录','已审核 npm 入口','已审核安装器 Node',
                         '@openai/codex', $fixture.prefix, $fixture.globalRoot, $fixture.package, $fixture.manager, $fixture.node)) {
        if ($names -cnotcontains $value) { throw 'Unconfirmed session target snapshot mismatch' }
    }
    Wait-InstallText '再次预览不会清理残留或自动重试。此记录不提供可重放令牌或恢复命令。' 'session-boundary'
    Wait-InstallText '请求可能在内部预检阶段被拒绝，也可能已执行或留下部分软件；仅凭失败不能判断。没有自动重试、回滚、清理或卸载。' 'session-boundary'
    $after = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'session-observation'
    foreach ($key in @('installCalls','versionProbes','prefixQueries','rootQueries')) {
        if ($before.$key -ne $after.$key) { throw 'Read-only session inspection executed a fixture operation' }
    }
}
function Initialize-InstallWindowActivation {
    if (-not ('AgentDeckAcceptanceForeground' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class AgentDeckAcceptanceForeground {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool SetForegroundWindow(IntPtr window);
}
'@
    }
}
function Get-InstallForegroundWindow {
    return [AgentDeckAcceptanceForeground]::GetForegroundWindow()
}
function Get-InstallWorkspaceInvoke($tab) {
    $pattern = $null
    if ($tab.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        return [Windows.Automation.InvokePattern]$pattern
    }
    return $null
}
function Prepare-InstallWorkspaceKeyboard($tab) {
    Initialize-InstallWindowActivation
    $script:running.Refresh()
    if ($script:running.HasExited) { throw 'Installed app stopped before navigation' }
    $handle = $script:running.MainWindowHandle
    if ($handle -eq [IntPtr]::Zero) { throw 'Installed app has no main window' }
    # One foreground request and one focus request, not an activation retry.
    $null = [AgentDeckAcceptanceForeground]::SetForegroundWindow($handle)
    $tab.SetFocus()
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        if ((Get-InstallForegroundWindow) -eq $handle -and $tab.Current.HasKeyboardFocus) { return $handle }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Installed workspace keyboard target was not confirmed'
}
function Send-InstallWorkspaceEnterKey {
    [Windows.Forms.SendKeys]::SendWait('{ENTER}')
}
function Send-InstallWorkspaceEnter($tab, [IntPtr]$handle) {
    # Recheck immediately before global keyboard input. UIA focus alone does not
    # establish the OS foreground recipient of SendKeys.
    if ($handle -eq [IntPtr]::Zero -or (Get-InstallForegroundWindow) -ne $handle -or -not $tab.Current.HasKeyboardFocus) {
        throw 'Installed workspace keyboard target changed before activation'
    }
    Send-InstallWorkspaceEnterKey
}
function Activate-InstallWorkspace([ValidateSet('Qwen 配置检查','Agent 管理','记忆管理')][string]$name) {
    $workspaceId = if ($name -eq 'Qwen 配置检查') { 'qwen' } elseif ($name -eq '记忆管理') { 'memory' } else { 'agents' }
    $route = 'unavailable'
    $focusConfirmed = $false
    $foregroundConfirmed = $false
    try {
        $tab = Wait-QwenControl $name ([Windows.Automation.ControlType]::TabItem)
        $invoke = Get-InstallWorkspaceInvoke $tab
        if ($null -ne $invoke) {
            $route = 'invoke'
            Record 'installed-workspace-activation' @{ workspace = $workspaceId; route = $route }
            $invoke.Invoke()
        } else {
            $route = 'keyboard'
            Add-Type -AssemblyName System.Windows.Forms
            $handle = Prepare-InstallWorkspaceKeyboard $tab
            $focusConfirmed = $tab.Current.HasKeyboardFocus
            $foregroundConfirmed = (Get-InstallForegroundWindow) -eq $handle
            Record 'installed-workspace-activation' @{ workspace = $workspaceId; route = $route; focusConfirmed = $focusConfirmed; foregroundConfirmed = $foregroundConfirmed }
            Send-InstallWorkspaceEnter $tab $handle
        }
        # An uncertain outcome is never replayed through another route.
        Wait-InstallWorkspaceSelected $name
    } catch {
        $activationFailure = $_
        Record 'installed-workspace-activation-failed' @{ workspace = $workspaceId; route = $route; focusConfirmed = $focusConfirmed; foregroundConfirmed = $foregroundConfirmed }
        try { Save-QwenEvidence 'failed-reviewed-install-navigation' }
        catch { Record 'installed-workspace-evidence-unavailable' @{ captured = $false } }
        throw $activationFailure
    }
}
function Wait-InstallWorkspaceSelected([ValidateSet('Qwen 配置检查','Agent 管理','记忆管理')][string]$name) {
    $workspaceId = if ($name -eq 'Qwen 配置检查') { 'qwen' } elseif ($name -eq '记忆管理') { 'memory' } else { 'agents' }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $revealAttempted = $false
    do {
        $tab = Find-QwenControl $name ([Windows.Automation.ControlType]::TabItem)
        $tabPresent = $null -ne $tab
        $selected = $tabPresent -and ([Windows.Automation.SelectionItemPattern]$tab.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Current.IsSelected
        $control = if ($name -eq 'Qwen 配置检查') {
            Find-QwenControl 'settings.json 绝对路径' ([Windows.Automation.ControlType]::Edit)
        } elseif ($name -eq '记忆管理') {
            Find-QwenControl '读取已保存选择' ([Windows.Automation.ControlType]::Button)
        } else {
            Find-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        }
        $controlPresent = $null -ne $control
        $visible = $controlPresent -and -not $control.Current.IsOffscreen
        if ($selected -and $visible) { return }
        if ($selected -and $controlPresent -and -not $visible -and -not $revealAttempted) {
            # Main content retains scroll position. Reveal one confirmed target
            # control without activating it, then still require visible UI.
            $revealAttempted = $true
            $scroll = $null
            if ($control.TryGetCurrentPattern([Windows.Automation.ScrollItemPattern]::Pattern, [ref]$scroll)) {
                ([Windows.Automation.ScrollItemPattern]$scroll).ScrollIntoView()
            } else {
                $control.SetFocus()
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    Record 'failed-reviewed-install-navigation-timeout' @{ workspace = $workspaceId; tabPresent = $tabPresent; selected = $selected; controlPresent = $controlPresent; visible = $visible; revealAttempted = $revealAttempted }
    throw 'Installed workspace selection or visible content did not settle'
}
function Assert-ResidualResultAbsent {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $text = Get-InstallAccessibleText
        if (-not $text.Contains('Codex 固定槽位只读检查结果') -and
            $null -eq (Find-QwenControl '关闭检查结果' ([Windows.Automation.ControlType]::Button))) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Residual inspection result remained after dismissal or navigation'
}
function Check-FailedResidualUnavailable($fixture, [string]$manifest, [string]$baseline, [string]$residual) {
    $before = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest) | ConvertFrom-Json
    Assert-ResidualResultAbsent
    # Hold the authored package directory with no sharing: product metadata reads
    # must fail closed. This handle neither reads nor writes directory contents.
    if (-not ('AgentDeckResidualAcceptanceLock' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class AgentDeckResidualAcceptanceLock {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern SafeFileHandle CreateFileW(string path, uint access, uint sharing,
        IntPtr security, uint creation, uint flags, IntPtr template);
}
'@
    }
    $handle = [AgentDeckResidualAcceptanceLock]::CreateFileW($fixture.package, 0x81, 0, [IntPtr]::Zero, 3, 0x02000000, [IntPtr]::Zero)
    try {
        if ($handle.IsInvalid) { throw 'Could not establish authored residual sharing conflict' }
        Click-Qwen '只读检查固定残留槽位'
        $fixedError = '本会话只读检查不可用或路径发生变化。未清理或执行程序。请保留现有文件。'
        Wait-InstallText $fixedError 'residual-unavailable'
        if ((Get-InstallAccessibleText).Contains('Codex 固定槽位只读检查结果')) { throw 'Unavailable residual inspection displayed success' }
        Save-QwenEvidence 'failed-reviewed-install-residual-unavailable'
        Click-Qwen '关闭检查结果'
        Assert-ResidualResultAbsent
    } finally { $handle.Dispose() }
    $after = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'residual-inspection'
    foreach ($key in @('installCalls','versionProbes','prefixQueries','rootQueries')) {
        if ($before.$key -ne $after.$key) { throw 'Unavailable residual inspection executed a fixture operation' }
    }
    Record 'failed-reviewed-install-residual-unavailable' @{ fixedSafeError = $true; residualUnchanged = $true; configurationUnchanged = $true; noFixtureExecution = $true; officialAgentExecuted = $false }
}
function Check-FailedInstallResidualInspection($fixture, [string]$manifest, [string]$baseline, [string]$residual, [switch]$KeepOpen) {
    $before = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest) | ConvertFrom-Json
    Assert-ResidualResultAbsent
    # Explicit native activation only. No DOM evaluation or direct backend call.
    Click-Qwen '只读检查固定残留槽位'
    $null = Wait-QwenControl '关闭检查结果' ([Windows.Automation.ControlType]::Button)
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $text = Get-InstallAccessibleText
        $complete = $true
        foreach ($expected in @('package_directory: directory','posix_launcher: missing','cmd_launcher: missing','powershell_launcher: missing')) {
            if (-not $text.Contains($expected)) { $complete = $false }
        }
        if ($complete) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $complete) { throw 'Fixed residual slot states were not observed in installed UI' }
    Wait-InstallText '未运行 npm 或 Agent；所有权未知；不支持清理。' 'residual-boundary'
    $after = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'residual-inspection'
    foreach ($key in @('installCalls','versionProbes','prefixQueries','rootQueries')) {
        if ($before.$key -ne $after.$key) { throw 'Residual inspection executed a fixture operation' }
    }
    Save-QwenEvidence 'failed-reviewed-install-residual-inspection'
    Record 'failed-reviewed-install-residual-inspection' @{ slots = @('directory','missing','missing','missing'); residualUnchanged = $true; configurationUnchanged = $true; noFixtureExecution = $true; officialAgentExecuted = $false; cleanupSupported = $false; ownership = 'unknown' }
    if ($KeepOpen) { return }
    Click-Qwen '关闭检查结果'
    Assert-ResidualResultAbsent
    # Closing must not auto-reopen a result or perform a second inspection.
    Start-Sleep -Milliseconds 300
    Assert-ResidualResultAbsent
}
function Invoke-InstallPrivateHome([string]$name, [scriptblock]$action) {
    # Isolate independent installation scenarios, but preserve each scenario's
    # exact HOME and PATH across its own real restart. Never clear a product guard.
    Stop-App
    $savedHome = $script:homeDir; $savedSentinel = $script:sentinel
    $saved = @{}
    foreach ($key in @('HOME','USERPROFILE','AGENTDECK_TEST_HOME','APPDATA','LOCALAPPDATA')) { $saved[$key] = [Environment]::GetEnvironmentVariable($key) }
    try {
        $script:homeDir = Join-Path $root $name
        New-Item -ItemType Directory -Path $script:homeDir | Out-Null
        $env:HOME = $script:homeDir; $env:USERPROFILE = $script:homeDir; $env:AGENTDECK_TEST_HOME = $script:homeDir
        $env:APPDATA = Join-Path $script:homeDir 'AppData\Roaming'; $env:LOCALAPPDATA = Join-Path $script:homeDir 'AppData\Local'
        New-Item -ItemType Directory -Force -Path $env:APPDATA,$env:LOCALAPPDATA | Out-Null
        $script:sentinel = Join-Path $script:homeDir '.agentdeck\acceptance-user-data.txt'
        New-Item -ItemType Directory -Path (Split-Path $script:sentinel) | Out-Null
        [IO.File]::WriteAllText($script:sentinel, $script:sentinelValue)
        & $action
    } finally {
        Stop-App
        $script:homeDir = $savedHome; $script:sentinel = $savedSentinel
        foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key]) }
    }
}
function Open-BlockedRecovery {
    Click-Qwen '检查恢复探测条件'
    Wait-InstallText '当前条件不允许解除保护。保留文件并人工核对；不会执行 npm 或 Codex。' 'recovery-blocked'
    if ($null -ne (Find-QwenControl '确认恢复 Codex 探测' ([Windows.Automation.ControlType]::Button))) { throw 'Blocked recovery exposed confirmation' }
    Click-Qwen '取消恢复检查'
}
function Get-ProtectionDigest([string]$manifest, [string]$protectionHome = $homeDir) {
    $digest = (Invoke-InstallFixture @('--protection','snapshot','--manifest',$manifest,'--home',$protectionHome) | ConvertFrom-Json).digest
    $stateRoot = Join-Path $protectionHome '.agentdeck\install-protection'
    $security = foreach ($path in @($stateRoot,(Join-Path $stateRoot 'codex'),(Join-Path $stateRoot 'codex\gate.lock'),(Join-Path $stateRoot 'codex\state.json'))) { (Get-Acl -LiteralPath $path).Sddl }
    return ConvertTo-Json -InputObject @($digest,$security) -Depth 3 -Compress
}
function Assert-RecoveryNoExecution([string]$manifest, $expected) {
    $current = Invoke-InstallFixture @('--assert','empty-owned','--manifest',$manifest) | ConvertFrom-Json
    foreach ($key in @('installCalls','versionProbes','prefixQueries','rootQueries','metadataQueries')) {
        if ($current.$key -ne $expected.$key) { throw 'Recovery check executed a fixture operation' }
    }
}
function Get-ProtectionSecurityDifference([string]$expected, [string]$observed) {
    # Return fixed categories only; descriptors, account SIDs and paths stay private.
    $left = [Security.AccessControl.RawSecurityDescriptor]::new($expected)
    $right = [Security.AccessControl.RawSecurityDescriptor]::new($observed)
    if ($left.Owner -ne $right.Owner) { 'owner' }
    if ($left.Group -ne $right.Group) { 'group' }
    foreach ($part in @('DiscretionaryAcl','SystemAcl')) {
        $a = $left.$part; $b = $right.$part
        $different = ($null -eq $a) -ne ($null -eq $b)
        if ($null -ne $a -and $null -ne $b) {
            $aBytes = [byte[]]::new($a.BinaryLength); $a.GetBinaryForm($aBytes, 0)
            $bBytes = [byte[]]::new($b.BinaryLength); $b.GetBinaryForm($bBytes, 0)
            $different = [Convert]::ToBase64String($aBytes) -cne [Convert]::ToBase64String($bBytes)
        }
        if ($different) { if ($part -eq 'DiscretionaryAcl') { 'dacl' } else { 'sacl' } }
    }
    if ($left.ControlFlags -ne $right.ControlFlags) {
        'control'
        $changed = [int]$left.ControlFlags -bxor [int]$right.ControlFlags
        foreach ($flag in [Enum]::GetValues([Security.AccessControl.ControlFlags])) {
            if ([int]$flag -ne 0 -and ($changed -band [int]$flag) -ne 0) { 'control:' + $flag.ToString() }
        }
    }
    if ($left.ResourceManagerControl -ne $right.ResourceManagerControl) { 'resource-manager-control' }
    # A differing complete SDDL remains a failure even if its parsed parts match.
    if ($expected -cne $observed) { 'sddl' }
}
function Check-SyntheticUncertainProtection([string]$manifest, $expected) {
    # These are test-authored record mutations, not evidence of an actual crash.
    # Slots are empty, so armed refusal cannot pass merely because residue exists.
    foreach ($case in @('armed','corrupt')) {
        Stop-App
        $sourceHome = $script:homeDir
        $destination = Join-Path $root ('Synthetic protection ' + $case)
        New-Item -ItemType Directory -Path $destination | Out-Null
        $null = Invoke-InstallFixture @('--synthetic-protection',$case,'--manifest',$manifest,'--destination-home',$destination)
        # Clone only this user's already-private fixture protection descriptors;
        # inherited default ACLs would make a synthetic armed test meaningless.
        foreach ($relative in @('.agentdeck\install-protection','.agentdeck\install-protection\codex','.agentdeck\install-protection\codex\gate.lock','.agentdeck\install-protection\codex\state.json')) {
            $source = Join-Path $sourceHome $relative; $target = Join-Path $destination $relative
            $acl = Get-Acl -LiteralPath $source
            Set-Acl -LiteralPath $target -AclObject $acl
            $observedSddl = (Get-Acl -LiteralPath $target).Sddl
            if ($observedSddl -cne $acl.Sddl) {
                Record 'synthetic-protection-security-mismatch' @{ case = $case; fields = @(Get-ProtectionSecurityDifference $acl.Sddl $observedSddl) }
                throw 'Synthetic protection security did not match its source'
            }
        }
        $before = Get-ProtectionDigest $manifest $destination
        $saved = @{}
        foreach ($key in @('HOME','USERPROFILE','AGENTDECK_TEST_HOME','APPDATA','LOCALAPPDATA')) { $saved[$key] = [Environment]::GetEnvironmentVariable($key) }
        try {
            $script:homeDir = $destination
            $env:HOME = $destination; $env:USERPROFILE = $destination; $env:AGENTDECK_TEST_HOME = $destination
            $env:APPDATA = Join-Path $destination 'AppData\Roaming'; $env:LOCALAPPDATA = Join-Path $destination 'AppData\Local'
            New-Item -ItemType Directory -Force -Path $env:APPDATA,$env:LOCALAPPDATA | Out-Null
            Check-Gui ('synthetic-protection-' + $case) -KeepOpen
            Set-QwenPath '搜索 Agent' 'Codex'
            Wait-InstallText 'Codex 安装保护未通过检查，已暂停此条目的主动探测。' 'probe-suppression'
            Click-Qwen '检查恢复探测条件'
            if ($case -eq 'armed') {
                Wait-InstallText '当前条件不允许解除保护。保留文件并人工核对；不会执行 npm 或 Codex。' 'recovery-blocked'
            } else {
                Wait-InstallText '恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。' 'recovery-error'
            }
            if ($null -ne (Find-QwenControl '确认恢复 Codex 探测' ([Windows.Automation.ControlType]::Button))) { throw 'Uncertain synthetic protection offered recovery' }
            Click-Qwen '刷新列表'
            $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
            Start-Sleep -Milliseconds 1000
            Assert-RecoveryNoExecution $manifest $expected
            if ((Get-ProtectionDigest $manifest $destination) -cne $before) { throw 'Uncertain synthetic protection state changed' }
            Save-QwenEvidence ('synthetic-protection-' + $case + '-retained')
            Record ('synthetic-protection-' + $case + '-retained') @{ emptySlots = $true; noFixtureExecution = $true; stateUnchanged = $true; actualCrashVerified = $false }
        } finally {
            Stop-App; $script:homeDir = $sourceHome
            foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key]) }
        }
    }
}
function ConvertTo-ExpectedRecoveryPrefix([string]$value) {
    # Match Store::normalize's one permitted Windows spelling conversion only.
    # Do not resolve paths, fold case, accept UNC/device roots or fuzzy-match.
    if ($value.StartsWith('\\?\',[StringComparison]::Ordinal)) {
        $rest = $value.Substring(4)
        if ($rest -cnotmatch '^[A-Za-z]:\\') { throw 'Unsupported recovery prefix spelling' }
        return $rest
    }
    if ($value -cnotmatch '^[A-Za-z]:\\') { throw 'Unsupported recovery prefix spelling' }
    return $value
}
function Assert-RecoveryEligibleTarget($fixture) {
    $null = Wait-QwenControl '确认恢复 Codex 探测' ([Windows.Automation.ControlType]::Button)
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    $regions = $window.FindAll([Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, 'Codex 安装保护恢复检查'))
    if ($regions.Count -ne 1) { throw 'Missing or ambiguous recovery preview' }
    $nodes = $regions[0].FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    $names = @($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
    $text = $names -join "`n"
    $expectedPrefix = ConvertTo-ExpectedRecoveryPrefix $fixture.prefix
    if ($names -cnotcontains ('已记录安装位置：' + $expectedPrefix)) { throw 'Recovery preview target mismatch' }
    foreach ($slot in @('package_directory','posix_launcher','cmd_launcher','powershell_launcher','exe_launcher','com_launcher','bat_launcher')) {
        if ($names -cnotcontains $slot) { throw 'Recovery preview slot missing' }
    }
    if (@($names | Where-Object { $_ -ceq 'missing' }).Count -lt 7 -or $names -ccontains 'present' -or $names -ccontains 'blocked') { throw 'Recovery preview slots not absent' }
    if (-not $text.Contains('确认只解除探测保护；之后刷新可恢复 npm/Codex 查询。不安装、不删除，也不代表包真实性或运行验证。')) { throw 'Recovery consent boundary missing' }
}
function Check-ExplicitInstallRecovery($fixture,[string]$manifest) {
    $null = Invoke-InstallFixture @('--relocate-owned','--manifest',$manifest)
    $expected = Invoke-InstallFixture @('--assert','empty-owned','--manifest',$manifest) | ConvertFrom-Json
    Check-SyntheticUncertainProtection $manifest $expected
    Check-Gui 'recovery-original-home-resumed' -KeepOpen
    Set-QwenPath '搜索 Agent' 'Codex'
    Wait-InstallText 'Codex 安装保护未通过检查，已暂停此条目的主动探测。' 'probe-suppression'
    $before = Get-ProtectionDigest $manifest
    Click-Qwen '检查恢复探测条件'
    Assert-RecoveryEligibleTarget $fixture
    Click-Qwen '取消恢复检查'
    Assert-RecoveryNoExecution $manifest $expected
    if ((Get-ProtectionDigest $manifest) -cne $before) { throw 'Cancelled recovery changed protection' }
    Click-Qwen '检查恢复探测条件'
    Assert-RecoveryEligibleTarget $fixture
    $null = Invoke-InstallFixture @('--conflict','create','--manifest',$manifest)
    Click-Qwen '确认恢复 Codex 探测'
    Wait-InstallText '恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。' 'recovery-error'
    if ((Get-ProtectionDigest $manifest) -cne $before) { throw 'Stale recovery cleared protection' }
    $null = Invoke-InstallFixture @('--conflict','remove','--manifest',$manifest)
    Assert-RecoveryNoExecution $manifest $expected
    Click-Qwen '检查恢复探测条件'
    Assert-RecoveryEligibleTarget $fixture
    Click-Qwen '确认恢复 Codex 探测'
    Wait-InstallText '已解除 Codex 探测保护，未安装或清理软件。' 'recovery-cleared'
    $null = Invoke-InstallFixture @('--protection','clear','--manifest',$manifest)
    # Active npm path queries may resume only now, after this explicit consent.
    $counts = Invoke-InstallFixture @('--assert','empty-owned','--manifest',$manifest) | ConvertFrom-Json
    if ($counts.installCalls -ne 1 -or $counts.versionProbes -ne 0) { throw 'Recovery installed or ran the authored Agent' }
    Save-QwenEvidence 'reviewed-install-recovery-cleared'
    Open-ReviewedInstallPlanAfterRecovery $fixture $manifest
    Click-Qwen '确认安装软件'
    Wait-InstallText '安装请求已发送，但完成状态未确认。' 'authored-install-error'
    Wait-InstallText 'Codex 安装保护未通过检查，已暂停此条目的主动探测。' 'probe-suppression'
    $counts = Invoke-InstallFixture @('--assert','wrong-root','--manifest',$manifest,'--expected-attempts','2') | ConvertFrom-Json
    $null = Invoke-InstallFixture @('--protection','failed_returned','--manifest',$manifest)
    Record 'reviewed-install-recovery-complete' @{ fixtureCounts = $counts; cancelUnchanged = $true; staleRejected = $true; explicitRecoveryConfirmed = $true; freshInstallationConfirmed = $true; testAuthoredResidueRelocation = $true; productCleanup = $false; secondWrongRootRejected = $true; authoredAgentExecuted = $false }
}
function Open-ReviewedInstallPlanAfterRecovery($fixture,[string]$manifest) {
    # Original Open-ReviewedInstallPlan asserts a never-attempted fixture, whereas
    # this fixture retains its first attempt's event log and archived residue.
    Click-Qwen '安装 OpenAI Codex'
    $null = Wait-QwenControl '确认安装软件' ([Windows.Automation.ControlType]::Button)
    $names = Get-InstallPlanNames
    foreach ($value in @('@openai/codex','1.2.3',$fixture.prefix,$fixture.package,$fixture.manager)) {
        if ($names -cnotcontains $value) { throw 'Fresh post-recovery installation preview mismatch' }
    }
    $null = Invoke-InstallFixture @('--assert','empty-owned','--manifest',$manifest)
}
function Assert-WrongRootInstallObservation($fixture, [string]$manifest, [string]$baseline, [string]$residual,
    [ValidateSet('initial','automatic-refresh','manual-refresh','navigation','restart')][string]$stage, $expectedCounts = $null) {
    $counts = Invoke-InstallFixture @('--assert', 'wrong-root', '--manifest', $manifest) | ConvertFrom-Json
    if ($counts.installCalls -ne 1 -or $counts.versionProbes -ne 0 -or $counts.completeWrongRootRetained -ne $true) {
        throw 'Wrong-root installation was probed, retried or lost its complete residue'
    }
    if ($null -ne $expectedCounts) {
        foreach ($key in @('installCalls','versionProbes','prefixQueries','rootQueries','metadataQueries')) {
            if ($counts.$key -ne $expectedCounts.$key) { throw 'Suppressed wrong-root observation executed a fixture operation' }
        }
    }
    $current = Get-FailedInstallState $fixture
    if ($current -cne $residual) {
        Record 'wrong-root-install-residual-changed' @{ stage = $stage; differences = @(Get-FailedStateDifference $residual $current) }
        throw 'Wrong-root installation residual changed'
    }
    if ((Get-InstallConfigurationSnapshot) -cne $baseline) { throw 'Wrong-root installation changed configuration' }
    Assert-InstallNoConfiguration
    Assert-InstallNoConfirmation
    Wait-InstallText 'Codex 安装保护未通过检查，已暂停此条目的主动探测。刷新或切换页面不会为此条目执行 Codex 或 npm；重启不代表安全恢复。未清理残留。' 'probe-suppression'
    $text = Get-InstallAccessibleText
    if ($text.Contains('软件安装操作已完成') -or $text.Contains('实测版本 1.2.3')) { throw 'Wrong-root installation displayed verified success' }
    foreach ($name in @('安装 OpenAI Codex','应用默认配置 OpenAI Codex')) {
        $action = Find-QwenControl $name ([Windows.Automation.ControlType]::Button)
        if ($null -ne $action -and $action.Current.IsEnabled) { throw 'Suppressed wrong-root action remained enabled' }
    }
    return $counts
}
function Check-WrongRootReviewedInstall {
    Invoke-InstallPrivateHome 'Wrong root protected home' { Check-WrongRootReviewedInstallCase }
}
function Check-WrongRootReviewedInstallCase {
    $savedPath = $env:PATH; $savedExt = $env:PATHEXT
    $fixtureRoot = Join-Path $root 'Windows authored wrong root install fixture'
    $manifest = Invoke-InstallFixture @('--prepare', $fixtureRoot, '--node', $script:acceptanceNode, '--home', $homeDir, '--mode', 'wrong-root')
    $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
    try {
        $env:PATH = "$($fixture.path);$savedPath"
        $env:PATHEXT = '.COM;.EXE;.BAT;.CMD'
        Check-Gui 'wrong-root-reviewed-install-start' -KeepOpen
        Set-QwenPath '搜索 Agent' 'Codex'
        $null = Wait-QwenControl '安装 OpenAI Codex' ([Windows.Automation.ControlType]::Button)
        Assert-InstallNoConfiguration
        $baseline = Get-InstallConfigurationSnapshot
        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '确认安装软件'
        Wait-InstallText '安装请求已发送，但完成状态未确认。' 'authored-install-error'
        Assert-InstallNoConfirmation
        $null = Invoke-InstallFixture @('--assert', 'wrong-root', '--manifest', $manifest)
        Wait-InstallText 'Codex 安装保护未通过检查，已暂停此条目的主动探测。刷新或切换页面不会为此条目执行 Codex 或 npm；重启不代表安全恢复。未清理残留。' 'probe-suppression'
        $residual = Get-FailedInstallState $fixture
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'initial'
        $initialCounts = $counts
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Start-Sleep -Milliseconds 1000
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'automatic-refresh' $initialCounts
        Save-QwenEvidence 'wrong-root-reviewed-install-automatic-refresh'
        Record 'wrong-root-reviewed-install-automatic-refresh' @{ fixtureCounts = $counts; residualUnchanged = $true }
        Click-Qwen '刷新列表'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Start-Sleep -Milliseconds 1000
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'manual-refresh' $initialCounts
        Save-QwenEvidence 'wrong-root-reviewed-install-manual-refresh'
        Activate-InstallWorkspace 'Qwen 配置检查'
        Activate-InstallWorkspace 'Agent 管理'
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'navigation' $initialCounts
        Save-QwenEvidence 'wrong-root-reviewed-install-navigation'
        $null = Invoke-InstallFixture @('--protection','failed_returned','--manifest',$manifest)
        $protectionBefore = Get-ProtectionDigest $manifest
        Stop-App
        Check-Gui 'wrong-root-reviewed-install-restarted' -KeepOpen
        Set-QwenPath '搜索 Agent' 'Codex'
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'restart' $initialCounts
        Open-BlockedRecovery
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'restart' $initialCounts
        if ((Get-ProtectionDigest $manifest) -cne $protectionBefore) { throw 'Restart or refused recovery changed persistent state' }
        Click-Qwen '刷新列表'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Start-Sleep -Milliseconds 1000
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'restart' $initialCounts
        Activate-InstallWorkspace 'Qwen 配置检查'
        Activate-InstallWorkspace 'Agent 管理'
        $counts = Assert-WrongRootInstallObservation $fixture $manifest $baseline $residual 'restart' $initialCounts
        Save-QwenEvidence 'wrong-root-persistent-restart-retained'
        Record 'wrong-root-reviewed-install-complete' @{ fixtureCounts = $counts; stages = @('verification_rejected','automatic_refresh_unprobed','manual_refresh_unprobed','navigation_unprobed'); residualUnchanged = $true; configurationUnchanged = $true; authoredNpmExitZero = $true; authoredAgentExecuted = $false; officialAgentExecuted = $false; processLocalOnly = $false; wrongRootSessionRestarted = $true; automaticRollbackObserved = $false; retryAttempted = $false }
        Check-ExplicitInstallRecovery $fixture $manifest
    } finally {
        # Every restart above retains the same HOME/PATH and owned prefix.
        Stop-App
        $env:PATH = $savedPath; $env:PATHEXT = $savedExt
    }
}
function Check-FailedReviewedInstall {
    Invoke-InstallPrivateHome 'Partial failure protected home' { Check-FailedReviewedInstallCase }
}
function Check-FailedReviewedInstallCase {
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
        $baseline = Get-InstallConfigurationSnapshot
        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '取消'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ($counts.installCalls -ne 0 -or $counts.versionProbes -ne 0) { throw 'Cancelled failure fixture executed installation' }
        if ((Get-InstallConfigurationSnapshot) -cne $baseline) { throw 'Cancelled failure fixture changed configuration' }
        Save-QwenEvidence 'failed-reviewed-install-cancel'
        Record 'failed-reviewed-install-cancel' @{ fixtureCounts = $counts; configurationUnchanged = $true }

        Open-ReviewedInstallPlan $fixture $manifest
        Click-Qwen '确认安装软件'
        Wait-InstallText '安装请求已发送，但完成状态未确认。' 'authored-install-error'
        Assert-InstallNoConfirmation
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        $null = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest)
        $residual = Get-FailedInstallState $fixture
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'initial'
        # Observe the settled failure for a bounded interval, without retrying any
        # action. The event log counts attempts before exclusive-marker rejection.
        Start-Sleep -Milliseconds 1000
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'settled'
        Observe-FailedInstallSession $fixture $manifest $baseline $residual
        Check-FailedResidualUnavailable $fixture $manifest $baseline $residual
        Check-FailedInstallResidualInspection $fixture $manifest $baseline $residual
        Save-QwenEvidence 'failed-reviewed-install-confirmed'
        Record 'failed-reviewed-install-confirmed' @{ fixtureCounts = $counts; residualUnchanged = $true; noSuccessNotice = $true }

        Click-Qwen '刷新列表'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Start-Sleep -Milliseconds 1000
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'refresh'
        Observe-FailedInstallSession $fixture $manifest $baseline $residual
        Assert-ResidualResultAbsent
        Check-FailedInstallResidualInspection $fixture $manifest $baseline $residual -KeepOpen
        Save-QwenEvidence 'failed-reviewed-install-refreshed'
        Record 'failed-reviewed-install-refreshed' @{ fixtureCounts = $counts; residualUnchanged = $true; configurationUnchanged = $true }

        Activate-InstallWorkspace 'Qwen 配置检查'
        Activate-InstallWorkspace 'Agent 管理'
        Observe-FailedInstallSession $fixture $manifest $baseline $residual
        Assert-ResidualResultAbsent
        Save-QwenEvidence 'failed-reviewed-install-session-navigation'
        Record 'failed-reviewed-install-session-navigation' @{ reviewedTargetPreserved = $true; noFixtureExecution = $true; persistentRecovery = $false }

        $null = Invoke-InstallFixture @('--protection','failed_returned','--manifest',$manifest)
        $partialProtectionBefore = Get-ProtectionDigest $manifest
        $restartCounts = Invoke-InstallFixture @('--assert','failed','--manifest',$manifest) | ConvertFrom-Json
        Stop-App
        Check-Gui 'failed-reviewed-install-restart' -KeepOpen
        Set-QwenPath '搜索 Agent' 'Codex'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Assert-InstallSessionAbsent
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'restart'
        Save-QwenEvidence 'failed-reviewed-install-restarted'
        # Persistent protection keeps installation disabled after restart.
        # Recovery must also reject occupied slots without erasing them.
        Open-BlockedRecovery
        $action = Find-QwenControl '安装 OpenAI Codex' ([Windows.Automation.ControlType]::Button)
        if ($null -ne $action -and $action.Current.IsEnabled) { throw 'Persistent partial failure allowed installation' }
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'recovery-blocked'
        Assert-InstallSessionAbsent
        foreach ($key in @('installCalls','versionProbes','prefixQueries','rootQueries','metadataQueries')) {
            if ($counts.$key -ne $restartCounts.$key) { throw 'Persistent partial failure restart or recovery executed fixture operation' }
        }
        if ((Get-ProtectionDigest $manifest) -cne $partialProtectionBefore) { throw 'Partial failure restart or recovery changed protection' }
        Save-QwenEvidence 'failed-reviewed-install-recovery-blocked'
        Record 'failed-reviewed-install-recovery-blocked' @{ fixtureCounts = $counts; confirmationAbsent = $true; residualUnchanged = $true }
        Record 'failed-reviewed-install-complete' @{ fixtureCounts = $counts; stages = @('preview_cancel','confirmed_failure','refresh_no_retry','session_navigation_retained','explicit_residual_inspection','residual_unavailable_no_write','residual_dismiss_and_recheck','restart_no_retry','session_restart_absent','persistent_recovery_blocked'); residualUnchanged = $true; configurationUnchanged = $true; authoredNpmExecuted = $true; authoredAgentExecuted = $false; officialAgentExecuted = $false; automaticRollbackObserved = $false; retryAttempted = $false }
    } finally {
        Stop-App
        $env:PATH = $savedPath; $env:PATHEXT = $savedExt
    }
}
