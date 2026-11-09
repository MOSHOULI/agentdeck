# Dot-sourced by windows-installer-acceptance.ps1 after its disposable-runner guard.
# UIAutomation patterns operate the installed WebView. No JS evaluation or direct IPC.
function Find-QwenControl([string]$name, [Windows.Automation.ControlType]$type) {
    if ($null -eq $script:running -or $script:running.HasExited) { throw 'Installed app is not running' }
    $processCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $processCondition)
    if ($null -eq $window) { return $null }
    $condition = [Windows.Automation.AndCondition]::new(
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $name),
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, $type))
    $matches = $window.FindAll([Windows.Automation.TreeScope]::Descendants, $condition)
    if ($matches.Count -gt 1) { throw "Ambiguous accessible control: $name" }
    if ($matches.Count -eq 1) { return $matches[0] }
    return $null
}
function Wait-QwenControl([string]$name, [Windows.Automation.ControlType]$type) {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        try {
            $control = Find-QwenControl $name $type
            if ($null -ne $control -and $control.Current.IsEnabled) { return $control }
        } catch [Windows.Automation.ElementNotAvailableException] { }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for enabled UI control: $name"
}
function Click-Qwen([string]$name) {
    $control = Wait-QwenControl $name ([Windows.Automation.ControlType]::Button)
    # Invoke is the native accessibility equivalent of activation, not a backend shortcut.
    ([Windows.Automation.InvokePattern]$control.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
}
function Set-QwenPath([string]$name, [string]$value) {
    $control = Wait-QwenControl $name ([Windows.Automation.ControlType]::Edit)
    ([Windows.Automation.ValuePattern]$control.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).SetValue($value)
}
function Wait-QwenText([string]$name) {
    $null = Wait-QwenControl $name ([Windows.Automation.ControlType]::Text)
}
function Assert-QwenNoConfirmation {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $present = $false
        foreach ($name in @('确认应用 Vim 变更','确认撤销 Vim 设置')) {
            if ($null -ne (Find-QwenControl $name ([Windows.Automation.ControlType]::Button))) { $present = $true }
        }
        if (-not $present) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Stale confirmation remains'
}
function Save-QwenEvidence([string]$stage) {
    $processCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $processCondition)
    if ($null -eq $window) { throw 'App window missing during evidence check' }
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    $names = @($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
    # Check before writing evidence: never emit leaked configuration values.
    if (($names -join "`n").Contains('SYNTHETIC-NOT-A-CREDENTIAL')) { throw 'Configuration canary leaked in accessibility tree' }
    foreach ($node in $nodes) {
        $pattern = $null
        if ($node.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern, [ref]$pattern) -and
            ([Windows.Automation.ValuePattern]$pattern).Current.Value.Contains('SYNTHETIC-NOT-A-CREDENTIAL')) {
            throw 'Configuration canary leaked in accessible value'
        }
    }
    $names | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "$stage-accessibility.json") -Encoding utf8
}
function Open-QwenFixture([string]$settings, [string]$manifest) {
    $tab = Wait-QwenControl 'Qwen 配置检查' ([Windows.Automation.ControlType]::TabItem)
    ([Windows.Automation.SelectionItemPattern]$tab.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()
    Set-QwenPath 'settings.json 绝对路径' $settings
    Set-QwenPath '运行时 package.json 绝对路径' $manifest
}
function Enable-QwenVim {
    $control = Wait-QwenControl '启用 Vim 模式' ([Windows.Automation.ControlType]::CheckBox)
    $toggle = [Windows.Automation.TogglePattern]$control.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    if ($toggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) { throw 'Unexpected initial Vim target' }
    $toggle.Toggle()
}
function Assert-QwenHash([string]$path, [string]$expected) {
    if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path).Hash -ne $expected) { throw 'Qwen synthetic file changed unexpectedly' }
}
function Assert-QwenDocument([string]$settings, [bool]$enabled) {
    $doc = [IO.File]::ReadAllText($settings) | ConvertFrom-Json -AsHashtable
    # Assert both the managed field and every unrelated synthetic value, plus key counts.
    if ($doc.Count -ne 3 -or $doc['$version'] -ne 4 -or $doc.general.Count -ne 2 -or
        $doc.general.vimMode -isnot [bool] -or $doc.general.vimMode -ne $enabled -or
        $doc.general.other -ne 'preserve-me' -or $doc.security.Count -ne 1 -or
        $doc.security.token -ne 'SYNTHETIC-NOT-A-CREDENTIAL') { throw 'Wrong Qwen settings semantics' }
}
function Check-QwenLifecycle {
    $fixture = Join-Path $homeDir 'Qwen 合成 配置'
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $settings = Join-Path $fixture 'settings.json'
    $manifest = Join-Path $fixture 'package.json'
    $receipt = Join-Path $fixture '.agentdeck-qwen-vim.json'
    $lock = Join-Path $fixture '.agentdeck-qwen-vim.lock'
    $original = '{"$version":4,"general":{"vimMode":false,"other":"preserve-me"},"security":{"token":"SYNTHETIC-NOT-A-CREDENTIAL"}}'
    [IO.File]::WriteAllText($settings, $original)
    [IO.File]::WriteAllText($manifest, '{"name":"@qwen-code/qwen-code","version":"0.27.0"}')
    $originalHash = (Get-FileHash -LiteralPath $settings).Hash
    $manifestHash = (Get-FileHash -LiteralPath $manifest).Hash
    Open-QwenFixture $settings $manifest
    Enable-QwenVim
    Click-Qwen '预览 Vim 变更'
    $null = Wait-QwenControl '确认应用 Vim 变更' ([Windows.Automation.ControlType]::Button)
    Assert-QwenHash $settings $originalHash
    if ((Test-Path -LiteralPath $receipt) -or (Test-Path -LiteralPath $lock)) { throw 'Preview created ownership state' }
    Save-QwenEvidence 'qwen-preview'
    Click-Qwen '取消 Vim 变更'
    $null = Wait-QwenControl '预览 Vim 变更' ([Windows.Automation.ControlType]::Button)
    Assert-QwenNoConfirmation
    Assert-QwenHash $settings $originalHash
    Click-Qwen '预览 Vim 变更'
    $null = Wait-QwenControl '确认应用 Vim 变更' ([Windows.Automation.ControlType]::Button)
    Click-Qwen '清除路径与结果'
    # Wait for React to commit the cleared edit before asserting removed confirmation.
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $input = Wait-QwenControl 'settings.json 绝对路径' ([Windows.Automation.ControlType]::Edit)
        $value = ([Windows.Automation.ValuePattern]$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value
        if ($value -eq '') { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($value -ne '') { throw 'Clear paths did not clear input' }
    Assert-QwenNoConfirmation
    Assert-QwenHash $settings $originalHash
    if ((Test-Path -LiteralPath $receipt) -or (Test-Path -LiteralPath $lock)) { throw 'Cancelled preview created ownership state' }
    Save-QwenEvidence 'qwen-preview-cancel-clear'
    Record 'qwen-preview-cancel-clear' @{ settingsUnchanged = $true; noReceiptOrLock = $true }

    Set-QwenPath 'settings.json 绝对路径' $settings
    Set-QwenPath '运行时 package.json 绝对路径' $manifest
    Click-Qwen '预览 Vim 变更'
    $null = Wait-QwenControl '确认应用 Vim 变更' ([Windows.Automation.ControlType]::Button)
    # Even semantically equivalent external edits must expire the preview token.
    [IO.File]::WriteAllText($settings, ($original + "`n"))
    $staleHash = (Get-FileHash -LiteralPath $settings).Hash
    Click-Qwen '确认应用 Vim 变更'
    Wait-QwenText '未能确认操作完成。请检查文件与所有权记录；不要假定配置未改变或重复提交。重新预览或处理冲突后再试。'
    Assert-QwenNoConfirmation
    Assert-QwenHash $settings $staleHash
    Assert-QwenHash $manifest $manifestHash
    if (Test-Path -LiteralPath $receipt) { throw 'Stale preview applied an ownership receipt' }
    Save-QwenEvidence 'qwen-stale-preview'
    Record 'qwen-stale-preview' @{ externalBytesPreserved = $true; noReceipt = $true }
    [IO.File]::WriteAllText($settings, $original)
    Click-Qwen '预览 Vim 变更'
    Click-Qwen '确认应用 Vim 变更'
    Wait-QwenText '所选文件的 Vim 模式字段已应用。尚未验证 Qwen 实际运行效果。'
    Assert-QwenDocument $settings $true
    Assert-QwenHash $manifest $manifestHash
    if (-not (Test-Path -LiteralPath $receipt)) { throw 'Apply did not create receipt' }
    $applied = [IO.File]::ReadAllText($settings)
    $appliedHash = (Get-FileHash -LiteralPath $settings).Hash
    $receiptHash = (Get-FileHash -LiteralPath $receipt).Hash
    if ([IO.File]::ReadAllText($receipt).Contains('SYNTHETIC-NOT-A-CREDENTIAL')) { throw 'Receipt leaked unrelated configuration' }
    Assert-QwenNoConfirmation
    Save-QwenEvidence 'qwen-ui-apply'
    Record 'qwen-ui-apply' @{ fieldEnabled = $true; unrelatedFieldsPreserved = $true; receiptCreated = $true }

    # Close/relaunch the actual installed app: recovery must not depend on UI memory.
    Stop-App
    Check-Gui 'qwen-owned-restart' -KeepOpen
    Open-QwenFixture $settings $manifest
    Click-Qwen '撤销受管 Vim 设置'
    Click-Qwen '取消撤销'
    $null = Wait-QwenControl '撤销受管 Vim 设置' ([Windows.Automation.ControlType]::Button)
    Assert-QwenNoConfirmation
    Assert-QwenHash $settings $appliedHash
    Assert-QwenHash $receipt $receiptHash
    Save-QwenEvidence 'qwen-restart-restore-cancel'
    Record 'qwen-restart-restore-cancel' @{ settingsAndReceiptUnchanged = $true }

    # Simulate an external editor changing the owned field after confirmation opens.
    Click-Qwen '撤销受管 Vim 设置'
    $null = Wait-QwenControl '确认撤销 Vim 设置' ([Windows.Automation.ControlType]::Button)
    # Different bytes from the exact original ensure this is drift, not already-restored cleanup.
    [IO.File]::WriteAllText($settings, ($original + "`n"))
    $driftHash = (Get-FileHash -LiteralPath $settings).Hash
    Click-Qwen '确认撤销 Vim 设置'
    Wait-QwenText '未能确认操作完成。请检查文件与所有权记录；不要假定配置未改变或重复提交。重新预览或处理冲突后再试。'
    Assert-QwenHash $settings $driftHash
    Assert-QwenHash $receipt $receiptHash
    Assert-QwenHash $manifest $manifestHash
    Save-QwenEvidence 'qwen-restore-conflict'
    Record 'qwen-restore-conflict' @{ externalEditPreserved = $true; receiptPreserved = $true }

    # Only the isolated fixture is reset to its exact applied image; no real user repair.
    [IO.File]::WriteAllText($settings, $applied)
    Click-Qwen '撤销受管 Vim 设置'
    Click-Qwen '确认撤销 Vim 设置'
    Wait-QwenText '受管字段已撤销。尚未验证 Qwen 实际运行效果。'
    Assert-QwenDocument $settings $false
    Assert-QwenHash $manifest $manifestHash
    if (Test-Path -LiteralPath $receipt) { throw 'Successful restore left receipt' }
    Assert-QwenNoConfirmation
    Save-QwenEvidence 'qwen-ui-restore'
    Record 'qwen-ui-restore' @{ originalBooleanRestored = $true; unrelatedFieldsPreserved = $true; receiptRemoved = $true }
    Stop-App
}
