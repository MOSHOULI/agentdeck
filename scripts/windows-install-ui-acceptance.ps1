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
    [ValidateSet('session-boundary','residual-boundary','residual-unavailable','occupied-preview-error','authored-install-error','stale-error','install-complete','installed-version','version','source','package','global','prefix','manager')][string]$check) {
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
        Wait-InstallText '安装请求已发送，但完成状态未确认。' 'stale-error'
        Assert-InstallNoConfirmation
        $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
        if ((Get-CodexSnapshot) -cne $baseline) { throw 'Stale installation changed configuration' }
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
    [ValidateSet('initial','settled','refresh','restart','occupied-preview','session-observation','residual-inspection')][string]$stage) {
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

        Stop-App
        Check-Gui 'failed-reviewed-install-restart' -KeepOpen
        Set-QwenPath '搜索 Agent' 'Codex'
        $null = Wait-QwenControl '刷新列表' ([Windows.Automation.ControlType]::Button)
        Assert-InstallSessionAbsent
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'restart'
        Save-QwenEvidence 'failed-reviewed-install-restarted'
        # A partial package occupies the selected destination. A fresh preview
        # must be rejected by production guards; never erase it to force a retry.
        Click-Qwen '安装 OpenAI Codex'
        Wait-InstallText '无法取得有效安装预览，请刷新后重新检查。' 'occupied-preview-error'
        $counts = Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'occupied-preview'
        Assert-InstallSessionAbsent
        Save-QwenEvidence 'failed-reviewed-install-occupied-preview'
        Record 'failed-reviewed-install-occupied-preview' @{ fixtureCounts = $counts; confirmationAbsent = $true; residualUnchanged = $true }
        Record 'failed-reviewed-install-complete' @{ fixtureCounts = $counts; stages = @('preview_cancel','confirmed_failure','refresh_no_retry','session_navigation_retained','explicit_residual_inspection','residual_unavailable_no_write','residual_dismiss_and_recheck','restart_no_retry','session_restart_absent','occupied_preview_rejected'); residualUnchanged = $true; configurationUnchanged = $true; authoredNpmExecuted = $true; authoredAgentExecuted = $false; officialAgentExecuted = $false; automaticRollbackObserved = $false; retryAttempted = $false }
    } finally {
        Stop-App
        $env:PATH = $savedPath; $env:PATHEXT = $savedExt
    }
}
