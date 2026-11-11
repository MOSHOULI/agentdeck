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
        foreach ($name in @('确认应用 Vim 变更','确认更新受管 Vim 值','确认撤销 Vim 设置')) {
            if ($null -ne (Find-QwenControl $name ([Windows.Automation.ControlType]::Button))) { $present = $true }
        }
        if (-not $present) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Stale confirmation remains'
}
function Assert-AcceptanceCanaryValues($name, $value) {
    foreach ($canary in @('SYNTHETIC-NOT-A-CREDENTIAL', 'AGENTDECK_SYNTHETIC_INSTALL_PRIVATE_CANARY')) {
        if (([string]$name).Contains($canary)) { throw 'Synthetic canary leaked in accessibility tree' }
        if (([string]$value).Contains($canary)) { throw 'Synthetic canary leaked in accessible value' }
    }
}
function Assert-AcceptanceCanariesAbsent($nodes) {
    # UIA Name and Value can be null. Only null strings become empty; provider
    # exceptions must still abort evidence collection, never bypass this guard.
    foreach ($node in $nodes) {
        Assert-AcceptanceCanaryValues ([string]$node.Current.Name) $null
        $pattern = $null
        if ($node.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
            Assert-AcceptanceCanaryValues $null ([string]([Windows.Automation.ValuePattern]$pattern).Current.Value)
        }
    }
}
function Save-QwenEvidence([string]$stage) {
    $processCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
    $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $processCondition)
    if ($null -eq $window) { throw 'App window missing during evidence check' }
    $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    $names = @($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
    Assert-AcceptanceCanariesAbsent $nodes
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
function Assert-QwenDocument([string]$settings, [bool]$enabled, [string]$other = 'preserve-me') {
    $doc = [IO.File]::ReadAllText($settings) | ConvertFrom-Json -AsHashtable
    # Assert both the managed field and every unrelated synthetic value, plus key counts.
    if ($doc.Count -ne 3 -or $doc['$version'] -ne 4 -or $doc.general.Count -ne 2 -or
        $doc.general.vimMode -isnot [bool] -or $doc.general.vimMode -ne $enabled -or
        $doc.general.other -ne $other -or $doc.security.Count -ne 1 -or
        $doc.security.token -ne 'SYNTHETIC-NOT-A-CREDENTIAL') { throw 'Wrong Qwen settings semantics' }
}
function Check-QwenRecoveryObservation([string]$fixture, [string]$label, [string]$stage) {
    $before = @(Get-ChildItem -LiteralPath $fixture -Force | Sort-Object Name | ForEach-Object { "$($_.Name):$((Get-FileHash -LiteralPath $_.FullName).Hash)" }) | ConvertTo-Json -Compress
    Click-Qwen '只读检查 Vim 恢复状态'
    Wait-QwenText $label
    $after = @(Get-ChildItem -LiteralPath $fixture -Force | Sort-Object Name | ForEach-Object { "$($_.Name):$((Get-FileHash -LiteralPath $_.FullName).Hash)" }) | ConvertTo-Json -Compress
    if ($before -ne $after) { throw 'Read-only recovery observation changed fixture files' }
    Assert-QwenNoConfirmation
    Save-QwenEvidence $stage
    Record $stage @{ readOnly = $true; noAuthorization = $true }
}
function Get-QwenFixtureSnapshot([string]$fixture) {
    $items = @(Get-ChildItem -LiteralPath $fixture -Force | Sort-Object Name)
    return ConvertTo-Json -Depth 4 -Compress -InputObject @($items | ForEach-Object {
        $_.Refresh()
        if ($_.PSIsContainer -or ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Unexpected Qwen fixture entry' }
        @{ name = $_.Name; hash = (Get-FileHash -LiteralPath $_.FullName).Hash; length = $_.Length;
            modified = $_.LastWriteTimeUtc.Ticks; attributes = [int]$_.Attributes; acl = (Get-Acl -LiteralPath $_.FullName).Sddl }
    })
}
function Check-QwenManifestBoundary([string]$fixture, [string]$manifest) {
    $legacy = [IO.File]::ReadAllBytes($manifest)
    [IO.File]::WriteAllText($manifest, '{"name":"@qwen-code/qwen-code","version":"0.27.2"}')
    $before = Get-QwenFixtureSnapshot $fixture
    foreach ($action in @('recovery','update','restore')) {
        if ($action -eq 'recovery') {
            Click-Qwen '只读检查 Vim 恢复状态'
            Wait-QwenText '恢复诊断未完成。请检查所选文件与所有权记录；未获得有效观察结果。'
        } elseif ($action -eq 'update') {
            Click-Qwen '预览更新受管 Vim 值'
            Wait-QwenText '无法生成兼容预览。请检查所选文件、版本与受管记录。'
        } else {
            Click-Qwen '撤销受管 Vim 设置'
            Wait-QwenText '无法生成兼容预览。请检查所选文件、版本与受管记录。'
        }
        Assert-QwenNoConfirmation
        if ((Get-QwenFixtureSnapshot $fixture) -cne $before) { throw 'Manifest mismatch changed Qwen fixture or receipt' }
    }
    Save-QwenEvidence 'qwen-cross-version-receipt-rejected'
    Record 'qwen-cross-version-receipt-rejected' @{ fromVersion = '0.27.0'; toVersion = '0.27.2'; operations = @('recovery','update','restore'); unchanged = $true; qwenExecuted = $false }
    # Revert only our authored manifest bytes to resume legacy compatibility checks.
    [IO.File]::WriteAllBytes($manifest, $legacy)
}
function Preview-QwenRestore([string]$fixture) {
    $before = Get-QwenFixtureSnapshot $fixture
    Click-Qwen '撤销受管 Vim 设置'
    Wait-QwenText '当前值：启用；首次原值：关闭。'
    Wait-QwenText '动作：恢复首次原值并清理所有权记录。'
    $null = Wait-QwenControl '确认撤销 Vim 设置' ([Windows.Automation.ControlType]::Button)
    if ((Get-QwenFixtureSnapshot $fixture) -cne $before) { throw 'Restore preview changed fixture files' }
    Save-QwenEvidence 'qwen-restore-preview'
}
function Check-QwenCurrentLifecycle {
    $fixture = Join-Path $homeDir 'Qwen current synthetic'
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $settings = Join-Path $fixture 'settings.json'; $manifest = Join-Path $fixture 'package.json'
    $receipt = Join-Path $fixture '.agentdeck-qwen-vim.json'
    [IO.File]::WriteAllText($settings, '{"$version":4,"general":{"vimMode":false,"other":"preserve-me"},"security":{"token":"SYNTHETIC-NOT-A-CREDENTIAL"}}')
    [IO.File]::WriteAllText($manifest, '{"name":"@qwen-code/qwen-code","version":"0.27.2"}')
    $manifestHash = (Get-FileHash -LiteralPath $manifest).Hash
    Check-Gui 'qwen-current-start' -KeepOpen
    Open-QwenFixture $settings $manifest
    Enable-QwenVim
    Click-Qwen '预览 Vim 变更'; Click-Qwen '确认应用 Vim 变更'
    Wait-QwenText '所选文件的 Vim 模式字段已应用。尚未验证 Qwen 实际运行效果。'
    Assert-QwenDocument $settings $true
    $first = [IO.File]::ReadAllText($receipt) | ConvertFrom-Json
    if ($first.before -ne $false -or $first.after -ne $true -or $first.manifest_digest -cne $manifestHash.ToLowerInvariant()) { throw 'Current Qwen receipt did not bind original and manifest' }
    # Update both directions so final restore proves the first original is retained.
    foreach ($enabled in @($false, $true)) {
        $control = Wait-QwenControl '启用 Vim 模式' ([Windows.Automation.ControlType]::CheckBox)
        ([Windows.Automation.TogglePattern]$control.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)).Toggle()
        Click-Qwen '预览更新受管 Vim 值'; Click-Qwen '确认更新受管 Vim 值'
        Wait-QwenText '受管 Vim 值已更新；首次原值仍保留供撤销。尚未验证 Qwen 实际运行效果。'
        Assert-QwenDocument $settings $enabled
        $updated = [IO.File]::ReadAllText($receipt) | ConvertFrom-Json
        if ($updated.format -ne 2 -or $updated.before -ne $false -or $updated.after -ne $enabled -or $updated.before_digest -cne $first.before_digest -or $updated.manifest_digest -cne $first.manifest_digest) { throw 'Current Qwen update lost original receipt binding' }
        if ([IO.File]::ReadAllText($receipt).Contains('SYNTHETIC-NOT-A-CREDENTIAL')) { throw 'Current Qwen receipt leaked configuration' }
    }
    Preview-QwenRestore $fixture; Click-Qwen '确认撤销 Vim 设置'
    Wait-QwenText '受管字段已撤销。尚未验证 Qwen 实际运行效果。'
    Assert-QwenDocument $settings $false; Assert-QwenHash $manifest $manifestHash
    if (Test-Path -LiteralPath $receipt) { throw 'Current Qwen restore retained receipt' }
    Assert-QwenNoConfirmation
    Save-QwenEvidence 'qwen-current-lifecycle'
    Record 'qwen-current-lifecycle' @{ version = '0.27.2'; stages = @('apply','update_false','update_true','restore'); unrelatedFieldsPreserved = $true; firstOriginalPreserved = $true; qwenExecuted = $false }
    Stop-App
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
    Check-QwenRecoveryObservation $fixture '未发现所有权记录；不代表配置从未被修改。' 'qwen-recovery-no-receipt'
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

    Check-QwenManifestBoundary $fixture $manifest
    Check-QwenRecoveryObservation $fixture '观察到可撤销的受管字段；尚未执行撤销。' 'qwen-recovery-restore-field'

    # Close/relaunch the actual installed app: recovery must not depend on UI memory.
    Stop-App
    Check-Gui 'qwen-owned-restart' -KeepOpen
    Open-QwenFixture $settings $manifest
    $cancelSnapshot = Get-QwenFixtureSnapshot $fixture
    Preview-QwenRestore $fixture
    Click-Qwen '取消撤销'
    $null = Wait-QwenControl '撤销受管 Vim 设置' ([Windows.Automation.ControlType]::Button)
    Assert-QwenNoConfirmation
    Assert-QwenHash $settings $appliedHash
    Assert-QwenHash $receipt $receiptHash
    if ((Get-QwenFixtureSnapshot $fixture) -cne $cancelSnapshot) { throw 'Cancelled restore changed fixture' }
    Save-QwenEvidence 'qwen-restart-restore-cancel'
    Record 'qwen-restart-restore-cancel' @{ settingsAndReceiptUnchanged = $true }

    # Simulate an external editor changing the owned field after confirmation opens.
    Preview-QwenRestore $fixture
    $null = Wait-QwenControl '确认撤销 Vim 设置' ([Windows.Automation.ControlType]::Button)
    # Different bytes from the exact original ensure this is drift, not already-restored cleanup.
    [IO.File]::WriteAllText($settings, ($original + "`n"))
    $driftHash = (Get-FileHash -LiteralPath $settings).Hash
    $driftSnapshot = Get-QwenFixtureSnapshot $fixture
    Click-Qwen '确认撤销 Vim 设置'
    Wait-QwenText '未能确认操作完成。请检查文件与所有权记录；不要假定配置未改变或重复提交。重新预览或处理冲突后再试。'
    Assert-QwenHash $settings $driftHash
    Assert-QwenHash $receipt $receiptHash
    Assert-QwenHash $manifest $manifestHash
    Assert-QwenNoConfirmation
    if ((Get-QwenFixtureSnapshot $fixture) -cne $driftSnapshot) { throw 'Conflicting restoration changed fixture' }
    Save-QwenEvidence 'qwen-restore-conflict'
    Record 'qwen-restore-conflict' @{ externalEditPreserved = $true; receiptPreserved = $true }

    Check-QwenRecoveryObservation $fixture '受管字段存在冲突，当前观察不支持撤销。' 'qwen-recovery-conflict'
    [IO.File]::WriteAllText($settings, $original)
    Check-QwenRecoveryObservation $fixture '字段处于原状态，仅待清理所有权记录；尚未清理。' 'qwen-recovery-cleanup-receipt'
    if (-not (Test-Path -LiteralPath $receipt)) { throw 'Observation removed receipt' }

    # Only the isolated fixture is reset to its exact applied image; no real user repair.
    [IO.File]::WriteAllText($settings, $applied)
    $beforeClear = Get-QwenFixtureSnapshot $fixture
    Preview-QwenRestore $fixture
    Click-Qwen '清除路径与结果'
    Assert-QwenNoConfirmation
    foreach ($name in @('settings.json 绝对路径', '运行时 package.json 绝对路径')) {
        $control = Wait-QwenControl $name ([Windows.Automation.ControlType]::Edit)
        if (([Windows.Automation.ValuePattern]$control.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value -ne '') { throw 'Restore clear retained path' }
    }
    if ((Get-QwenFixtureSnapshot $fixture) -cne $beforeClear) { throw 'Cleared restore changed fixture' }
    Save-QwenEvidence 'qwen-restore-clear'
    Record 'qwen-restore-clear' @{ settingsAndReceiptUnchanged = $true; confirmationRemoved = $true }
    Set-QwenPath 'settings.json 绝对路径' $settings
    Set-QwenPath '运行时 package.json 绝对路径' $manifest
    Preview-QwenRestore $fixture
    # Authored synthetic external edit AFTER the UI obtains its snapshot token.
    $external = $applied | ConvertFrom-Json -AsHashtable
    $external.general.other = 'external-edit-preserved'
    [IO.File]::WriteAllText($settings, ($external | ConvertTo-Json -Depth 5 -Compress))
    $staleSnapshot = Get-QwenFixtureSnapshot $fixture
    Click-Qwen '确认撤销 Vim 设置'
    Wait-QwenText '未能确认操作完成。请检查文件与所有权记录；不要假定配置未改变或重复提交。重新预览或处理冲突后再试。'
    Assert-QwenNoConfirmation
    if ((Get-QwenFixtureSnapshot $fixture) -cne $staleSnapshot) { throw 'Stale restoration changed fixture or receipt' }
    Save-QwenEvidence 'qwen-restore-stale'
    Record 'qwen-restore-stale' @{ externalEditPreserved = $true; receiptPreserved = $true; noAutomaticRetry = $true }
    # Explicitly obtain a new preview after rejection; do not replay the old write.
    Preview-QwenRestore $fixture
    Click-Qwen '确认撤销 Vim 设置'
    Wait-QwenText '受管字段已撤销。尚未验证 Qwen 实际运行效果。'
    Assert-QwenDocument $settings $false 'external-edit-preserved'
    Assert-QwenHash $manifest $manifestHash
    if (Test-Path -LiteralPath $receipt) { throw 'Successful restore left receipt' }
    Assert-QwenNoConfirmation
    Save-QwenEvidence 'qwen-ui-restore'
    Record 'qwen-ui-restore' @{ originalBooleanRestored = $true; unrelatedFieldsPreserved = $true; receiptRemoved = $true }
    Stop-App
}
