# Installed Windows WebView controls over authored SQLite only. No real sessions,
# automatic collection, agent execution, JS evaluation, test plugin or direct IPC.
# Dot-source only after the hosted-runner guard and existing UIA helpers.
function Invoke-MemoryFixture([string[]]$arguments) {
    $savedConsoleEncoding = [Console]::OutputEncoding
    $savedOutputEncoding = $OutputEncoding
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $OutputEncoding = [Text.UTF8Encoding]::new($false)
        $output = & $script:acceptancePython -X utf8 (Join-Path $PSScriptRoot 'memory_export_fixture.py') --home $homeDir @arguments
        if ($LASTEXITCODE -ne 0) { throw 'Authored memory fixture guard failed' }
        return $output
    } finally {
        [Console]::OutputEncoding = $savedConsoleEncoding
        $OutputEncoding = $savedOutputEncoding
    }
}
function Get-MemorySnapshot([switch]$Unrelated) {
    # Fixture guard accepts only an empty WAL and a bounded ordinary shm file.
    # Read-only SQLite may create/remove these coordination files; their lock
    # metadata is not a product data write. Never ignore a nonempty WAL.
    $null = Invoke-MemoryFixture @('--snapshot')
    $state = Get-CodexSnapshot | ConvertFrom-Json -AsHashtable
    $base = Join-Path $homeDir '.agentdeck'
    $state.entries = @($state.entries | Where-Object {
        $_.path -notin @((Join-Path $base 'memory.db-wal'), (Join-Path $base 'memory.db-shm')) -and
        (-not $Unrelated -or -not $_.path.StartsWith((Join-Path $base 'memory'), [StringComparison]::OrdinalIgnoreCase))
    } | ForEach-Object {
        if ($_.path -eq $base) { $_.modified = 0; $_.length = 0 }
        $_
    })
    return ConvertTo-Json -InputObject $state -Depth 5 -Compress
}
function Assert-MemoryUnrelated {
    if ((Get-MemorySnapshot -Unrelated) -cne $script:memoryUnrelated) { throw 'Memory editor changed unrelated configuration' }
}
function Get-MemoryNodes {
    $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
    if ($null -eq $window) { throw 'Installed memory window missing' }
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    Assert-AcceptanceCanariesAbsent $nodes
    return $nodes
}
function Wait-MemoryText([string]$text) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        if ((Get-InstallAccessibleText).Contains($text)) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Expected authored memory UI state missing'
}
function Assert-MemoryNoPreview {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ($null -eq (Find-QwenControl '确认替换手动选择' ([Windows.Automation.ControlType]::Button)) -and
            $null -eq (Find-QwenControl '取消预览' ([Windows.Automation.ControlType]::Button))) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Memory confirmation survived invalidation'
}
function Assert-MemoryDraft([int[]]$ids) {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $actual = @((Get-MemoryNodes) | Where-Object { $_.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $_.Current.Name -match '^移除手动记忆 [0-9]+$' } | ForEach-Object { [int]($_.Current.Name -replace '^移除手动记忆 ', '') } | Sort-Object)
        if (($actual -join ',') -ceq (($ids | Sort-Object) -join ',')) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Memory draft IDs mismatch'
}
function Assert-MemoryQuiet([string]$before) {
    $null = Get-MemoryNodes
    Assert-MemoryUnrelated
    if ((Get-MemorySnapshot) -cne $before) { throw 'Unconfirmed memory operation changed bytes or metadata' }
    $collection = Wait-QwenControl '允许每小时自动采集本地会话' ([Windows.Automation.ControlType]::CheckBox)
    $toggle = [Windows.Automation.TogglePattern]$collection.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    if ($toggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) { throw 'Memory collection unexpectedly enabled' }
}
function Load-MemoryDraft([int[]]$ids) {
    Click-Qwen '读取已保存选择'
    Wait-MemoryText ("已保存手动 " + $ids.Count + ' 条；目标自动导出 ')
    Assert-MemoryDraft $ids
    $null = Wait-QwenControl '预览替换' ([Windows.Automation.ControlType]::Button)
    Assert-MemoryNoPreview
}
function Search-Memory([string]$query, [int[]]$ids) {
    Set-QwenPath '搜索本地记忆' $query
    Click-Qwen '搜索'
    Wait-MemoryText ("本次查询 " + $ids.Count + ' 条')
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $actual = @((Get-MemoryNodes) | Where-Object { $_.Current.ControlType -eq [Windows.Automation.ControlType]::CheckBox -and $_.Current.Name -match '^选择手动记忆 [0-9]+$' } | ForEach-Object { [int]($_.Current.Name -replace '^选择手动记忆 ', '') } | Sort-Object)
        if (($actual -join ',') -ceq (($ids | Sort-Object) -join ',')) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Memory search result IDs mismatch'
}
function Assert-MemorySelected([int]$id, [bool]$selected) {
    $expected = if ($selected) { [Windows.Automation.ToggleState]::On } else { [Windows.Automation.ToggleState]::Off }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $control = Wait-QwenControl "选择手动记忆 $id" ([Windows.Automation.ControlType]::CheckBox)
        $toggle = [Windows.Automation.TogglePattern]$control.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($toggle.Current.ToggleState -eq $expected) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Native memory selection state mismatch'
}
function Select-Memory([int]$id) {
    Assert-MemorySelected $id $false
    $control = Wait-QwenControl "选择手动记忆 $id" ([Windows.Automation.ControlType]::CheckBox)
    ([Windows.Automation.TogglePattern]$control.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)).Toggle()
    Assert-MemorySelected $id $true
}
function Prepare-MemoryProposal([int[]]$saved) {
    Load-MemoryDraft $saved
    foreach ($id in $saved) { if ($id -ne 2) { Click-Qwen "移除手动记忆 $id" } }
    Assert-MemoryDraft @(2)
    Search-Memory 'Harbor' @(4); Select-Memory 4; Assert-MemoryDraft @(2,4)
    Search-Memory 'Meadow' @(5); Select-Memory 5; Assert-MemoryDraft @(2,4,5)
    Search-Memory 'Harbor' @(4); Assert-MemorySelected 4 $true
    Assert-MemoryDraft @(2,4,5)
}
function Preview-Memory([int]$manual, [int]$automatic, [int]$total) {
    $before = Get-MemorySnapshot
    Click-Qwen '预览替换'
    $null = Wait-QwenControl '确认替换手动选择' ([Windows.Automation.ControlType]::Button)
    Wait-MemoryText "手动 $manual 条 + 自动 $automatic 条，去重后 $total 条。"
    Assert-MemoryQuiet $before
}
function Get-MemoryTargetValue($control) {
    $pattern = $null
    if ($control.TryGetCurrentPattern([Windows.Automation.SelectionPattern]::Pattern, [ref]$pattern)) {
        $selected = ([Windows.Automation.SelectionPattern]$pattern).GetCurrentSelection()
        if ($selected.Count -ne 1) { throw 'Memory target selection is ambiguous' }
        return $selected[0].Current.Name
    }
    if ($control.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
        return ([Windows.Automation.ValuePattern]$pattern).Current.Value
    }
    throw 'Native memory target observation unsupported'
}
function Select-MemoryTarget([ValidateSet('Codex','Claude','OpenCode')][string]$name) {
    $control = Wait-QwenControl '导出目标' ([Windows.Automation.ControlType]::ComboBox)
    $keys = switch ($name) { 'Codex' { '{HOME}{ENTER}' } 'Claude' { '{HOME}{DOWN}{ENTER}' } 'OpenCode' { '{END}{ENTER}' } }
    Add-Type -AssemblyName System.Windows.Forms
    $handle = Prepare-InstallWorkspaceKeyboard $control
    if ((Get-InstallForegroundWindow) -ne $handle -or -not $control.Current.HasKeyboardFocus) { throw 'Memory target keyboard recipient changed' }
    # One native key sequence only; any uncertain delivery aborts, never replays.
    [Windows.Forms.SendKeys]::SendWait($keys)
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ((Get-MemoryTargetValue $control) -ceq $name) {
            Wait-MemoryText ("当前导出目标：" + $name + ' · ~/.agentdeck/memory/')
            Wait-MemoryText '请读取当前目标的已保存选择后编辑。当前草稿不能提交。'
            Assert-MemoryDraft @(); Assert-MemoryNoPreview
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Memory target native selection did not settle'
}
function Save-MemoryStage([string]$stage) {
    $null = Get-MemoryNodes
    Assert-MemoryUnrelated
    Save-QwenEvidence ('memory-' + $stage)
    Record ('memory-' + $stage) @{ nativeInstalledWebView = $true; authoredSqliteOnly = $true; realAgentConsumptionVerified = $false }
    $script:memoryStages.Add($stage)
}
function Check-MemoryExport {
    $script:memoryStages = [Collections.Generic.List[string]]::new()
    $expected = @('saved_outside_recent','multisearch_reset','cancel_edit','target_changes','preview_cancel','stale_manual_rejected','stale_auto_rejected','stale_row_rejected','replace_union','clear_manual_retains_auto')
    try {
        $null = Invoke-MemoryFixture @('--prepare')
        Check-Gui 'memory-editor-start' -KeepOpen
        Activate-InstallWorkspace '记忆管理'
        Wait-MemoryText '自动采集已关闭'
        $script:memoryUnrelated = Get-MemorySnapshot -Unrelated
        $before = Get-MemorySnapshot
        Load-MemoryDraft @(1,2); Search-Memory '' @(17..66); Assert-MemoryDraft @(1,2)
        Assert-MemoryQuiet $before; Save-MemoryStage 'saved_outside_recent'

        $before = Get-MemorySnapshot
        Prepare-MemoryProposal @(1,2); Click-Qwen '重置草稿'; Assert-MemoryDraft @(1,2)
        Assert-MemoryQuiet $before; Save-MemoryStage 'multisearch_reset'

        $before = Get-MemorySnapshot
        Prepare-MemoryProposal @(1,2); Click-Qwen '取消编辑'
        Wait-MemoryText '请读取当前目标的已保存选择后编辑。当前草稿不能提交。'
        Assert-MemoryDraft @(); Assert-MemoryNoPreview
        Assert-MemoryQuiet $before; Save-MemoryStage 'cancel_edit'

        $before = Get-MemorySnapshot
        Load-MemoryDraft @(1,2); Preview-Memory 2 2 3
        Select-MemoryTarget 'Claude'; Load-MemoryDraft @(6)
        Select-MemoryTarget 'OpenCode'; Load-MemoryDraft @(5)
        Select-MemoryTarget 'Codex'; Load-MemoryDraft @(1,2)
        Assert-MemoryQuiet $before; Save-MemoryStage 'target_changes'

        $before = Get-MemorySnapshot
        Prepare-MemoryProposal @(1,2); Preview-Memory 3 2 4
        Click-Qwen '取消预览'; Assert-MemoryNoPreview; Assert-MemoryDraft @(2,4,5)
        Assert-MemoryQuiet $before; Save-MemoryStage 'preview_cancel'

        foreach ($case in @(@{kind='manual'; saved=@(1,2); automatic=2; total=4}, @{kind='auto'; saved=@(1,2,6); automatic=2; total=4}, @{kind='row'; saved=@(1,2,6); automatic=3; total=5})) {
            Prepare-MemoryProposal $case.saved; Preview-Memory 3 $case.automatic $case.total
            $null = Invoke-MemoryFixture @('--mutate', $case.kind)
            $before = Get-MemorySnapshot
            Click-Qwen '确认替换手动选择'
            Wait-MemoryText '导出状态未确认，请重新读取并预览。'
            Wait-MemoryText '已变化'
            Assert-MemoryNoPreview
            $previewControl = Find-QwenControl '预览替换' ([Windows.Automation.ControlType]::Button)
            if ($null -eq $previewControl -or $previewControl.Current.IsEnabled) { throw 'Stale memory preview remained enabled' }
            Assert-MemoryQuiet $before
            $null = Invoke-MemoryFixture @('--verify', ($case.kind + '-drift'))
            Save-MemoryStage ('stale_' + $case.kind + '_rejected')
        }

        Prepare-MemoryProposal @(1,2,6); Preview-Memory 3 3 5
        Click-Qwen '确认替换手动选择'
        Wait-MemoryText '已更新 Codex 本地 Markdown：手动 3 条，合计 5 条。不代表 Agent 已读取。'
        Assert-MemoryNoPreview
        $null = Invoke-MemoryFixture @('--verify', 'replaced')
        Load-MemoryDraft @(2,4,5); Save-MemoryStage 'replace_union'

        $before = Get-MemorySnapshot
        Click-Qwen '清空手动草稿'; Assert-MemoryDraft @(); Preview-Memory 0 3 3
        Assert-MemoryQuiet $before
        Click-Qwen '确认替换手动选择'
        Wait-MemoryText '已更新 Codex 本地 Markdown：手动 0 条，合计 3 条。不代表 Agent 已读取。'
        Assert-MemoryNoPreview
        $null = Invoke-MemoryFixture @('--verify', 'cleared')
        Load-MemoryDraft @(); Save-MemoryStage 'clear_manual_retains_auto'
        if (($script:memoryStages -join ',') -cne ($expected -join ',')) { throw 'Incomplete installed memory editor acceptance' }
        Record 'memory-export-complete' @{ stages = @($script:memoryStages); collectionEnabled = $false; realAgentConsumptionVerified = $false }
    } catch {
        $failure = $_
        try { Save-QwenEvidence 'memory-editor-failure' } catch { Record 'memory-evidence-unavailable' @{ captured = $false } }
        throw $failure
    } finally { Stop-App }
}
