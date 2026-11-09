# Dot-sourced only by the guarded disposable-runner installer acceptance script.
# Reuses native UIAutomation helpers; no JavaScript or direct IPC invocation.
function Check-QoderReadonly {
    $fixture = Join-Path $homeDir 'Qoder 合成 归档'
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $archive = Join-Path $fixture 'qodercli-linux-x64-baseline.tar.gz'
    [IO.File]::WriteAllText($archive, 'SYNTHETIC-NOT-A-CREDENTIAL')
    $before = (Get-FileHash -LiteralPath $archive).Hash
    $tab = Wait-QwenControl 'Qoder 分发与校验' ([Windows.Automation.ControlType]::TabItem)
    ([Windows.Automation.SelectionItemPattern]$tab.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()
    $combo = Wait-QwenControl '明确分发目标（不是本机自动检测）' ([Windows.Automation.ControlType]::ComboBox)
    # Real keyboard input into the focused HTML select. Placeholder + five explicit
    # entries selects linux-amd64-baseline. The actual plan below must prove it.
    Add-Type -AssemblyName System.Windows.Forms
    $combo.SetFocus()
    [Windows.Forms.SendKeys]::SendWait('{HOME}{DOWN}{DOWN}{DOWN}{DOWN}{DOWN}{ENTER}')
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
}
