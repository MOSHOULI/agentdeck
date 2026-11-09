# Dot-sourced only by the guarded disposable-runner installer acceptance script.
# Reuses native UIAutomation helpers; no JavaScript or direct IPC invocation.
function Assert-QoderSelectedTarget([string]$target) {
    Wait-QwenText "当前分发目标：$target"
    $combo = Wait-QwenControl '明确分发目标（不是本机自动检测）' ([Windows.Automation.ControlType]::ComboBox)
    $pattern = $null
    if ($combo.TryGetCurrentPattern([Windows.Automation.SelectionPattern]::Pattern, [ref]$pattern)) {
        $selected = @(([Windows.Automation.SelectionPattern]$pattern).GetCurrentSelection())
        if ($selected.Count -ne 1 -or $selected[0].Current.Name -ne $target) { throw 'Native Qoder selection disagrees with the explicit target' }
    } elseif ($combo.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
        if (([Windows.Automation.ValuePattern]$pattern).Current.Value -ne $target) { throw 'Native Qoder value disagrees with the explicit target' }
    }
    Record 'qoder-target-selected' @{ target = $target; visibleSelectionConfirmed = $true }
}
function Select-QoderTarget([string]$target) {
    $combo = Wait-QwenControl '明确分发目标（不是本机自动检测）' ([Windows.Automation.ControlType]::ComboBox)
    $expand = $null
    if ($combo.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$expand)) {
        ([Windows.Automation.ExpandCollapsePattern]$expand).Expand()
        $condition = [Windows.Automation.AndCondition]::new(
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $target),
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::ListItem))
        $options = $combo.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
        if ($options.Count -gt 1) { throw 'Ambiguous Qoder target option' }
        if ($options.Count -eq 1) {
            $selection = $null
            if ($options[0].TryGetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern, [ref]$selection)) {
                ([Windows.Automation.SelectionItemPattern]$selection).Select()
                ([Windows.Automation.ExpandCollapsePattern]$expand).Collapse()
                Assert-QoderSelectedTarget $target
                return
            }
        }
        ([Windows.Automation.ExpandCollapsePattern]$expand).Collapse()
    }
    # Some WebView builds expose native select popups outside the combo subtree.
    # Use real focused keyboard navigation then verify the app's visible selected
    # target after each step, rather than assuming an option index or ordering.
    Add-Type -AssemblyName System.Windows.Forms
    $combo.SetFocus()
    [Windows.Forms.SendKeys]::SendWait('{HOME}')
    for ($step = 0; $step -lt 12; $step++) {
        $deadline = [DateTime]::UtcNow.AddSeconds(1)
        do {
            if ($null -ne (Find-QwenControl "当前分发目标：$target" ([Windows.Automation.ControlType]::Text))) { Assert-QoderSelectedTarget $target; return }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        [Windows.Forms.SendKeys]::SendWait('{DOWN}')
    }
    throw "Could not select the exact Qoder target: $target"
}
function Check-QoderReadonly {
    try {
    $fixture = Join-Path $homeDir 'Qoder 合成 归档'
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $archive = Join-Path $fixture 'qodercli-linux-x64-baseline.tar.gz'
    [IO.File]::WriteAllText($archive, 'SYNTHETIC-NOT-A-CREDENTIAL')
    $before = (Get-FileHash -LiteralPath $archive).Hash
    $tab = Wait-QwenControl 'Qoder 分发与校验' ([Windows.Automation.ControlType]::TabItem)
    ([Windows.Automation.SelectionItemPattern]$tab.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()
    Select-QoderTarget 'linux-amd64-baseline'
    Click-Qwen '查看 Qoder 分发计划'
    Wait-QwenText '官方归档名：qodercli-linux-x64-baseline.tar.gz'
    Save-QwenEvidence 'qoder-fixed-distribution-plan'
    Assert-QwenHash $archive $before
    Set-QwenPath 'Qoder 官方归档绝对路径' $archive
    Click-Qwen '只读校验 Qoder 归档'
    Wait-QwenText '未能确认 Qoder 分发计划或包摘要。请检查目标、官方文件名、文件权限和支持的读取平台；没有获得校验成功结论。'
    Assert-QwenHash $archive $before
    if (@(Get-ChildItem -LiteralPath $fixture -Force).Count -ne 1) { throw 'Read-only Qoder check created files' }
    Save-QwenEvidence 'qoder-invalid-archive-rejected'
    Click-Qwen '清除 Qoder 选择与结果'
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $input = Wait-QwenControl 'Qoder 官方归档绝对路径' ([Windows.Automation.ControlType]::Edit)
        $value = ([Windows.Automation.ValuePattern]$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value
        if ($value -eq '') { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($value -ne '') { throw 'Qoder clear did not clear the selected archive path' }
    Assert-QwenHash $archive $before
    Record 'qoder-readonly-ui' @{ explicitPlan = $true; syntheticMismatchRejected = $true; filesUnchanged = $true; realArchiveSuccessNotTested = $true }
    Stop-App
    } catch {
        # Preserve the actual failure state before the installer runner stops the
        # app. The shared evidence writer checks names AND values for the canary.
        try { Save-QwenEvidence 'qoder-failure' } catch { Record 'qoder-failure-evidence-unavailable' 'Could not safely capture accessibility evidence' }
        throw
    }
}
