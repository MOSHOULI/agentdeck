# Dot-sourced only by the guarded disposable-runner installer acceptance script.
# Reuses native UIAutomation helpers; no JavaScript or direct IPC invocation.
function Assert-QoderSelectedTarget([string]$target) {
    Wait-QwenText "当前分发目标：$target"
    $radio = Wait-QwenControl $target ([Windows.Automation.ControlType]::RadioButton)
    $selection = [Windows.Automation.SelectionItemPattern]$radio.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)
    if (-not $selection.Current.IsSelected) { throw 'Native Qoder radio selection disagrees with the explicit target' }
    Record 'qoder-target-selected' @{ target = $target; visibleSelectionConfirmed = $true; nativeSelectionConfirmed = $true }
}
function Select-QoderTarget([string]$target) {
    # Real accessible radio selection, not keyboard guesses into an opaque popup.
    # Missing/ambiguous controls or unsupported patterns fail rather than skip.
    $radio = Wait-QwenControl $target ([Windows.Automation.ControlType]::RadioButton)
    ([Windows.Automation.SelectionItemPattern]$radio.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()
    Assert-QoderSelectedTarget $target
}
function Assert-QoderSuccessCleared {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $success = Find-QwenControl '归档与固定官方 SHA256 一致，仅证明所读包摘要匹配。' ([Windows.Automation.ControlType]::Text)
        $bytes = Find-QwenControl '已读取字节：78587775' ([Windows.Automation.ControlType]::Text)
        if ($null -eq $success -and $null -eq $bytes) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Stale Qoder verification survived an input change'
}
function Check-QoderRealArchive {
    $expected = 'a2fd8cc04711603f9637f54b63097380efc79939739967985cd6193507472dc4'
    $before = Get-Item -LiteralPath $RealQoderArchive
    if ($before.Length -ne 78587775 -or ($before.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Real Qoder fixture is not the expected regular archive' }
    $mtime = $before.LastWriteTimeUtc.Ticks
    $parent = Split-Path -Parent $RealQoderArchive
    if (@(Get-ChildItem -LiteralPath $parent -Force).Count -ne 1) { throw 'Unexpected contents in dedicated Qoder fixture directory' }
    Assert-QwenHash $RealQoderArchive $expected
    Set-QwenPath 'Qoder 官方归档绝对路径' $RealQoderArchive
    Click-Qwen '只读校验 Qoder 归档'
    Wait-QwenText '归档与固定官方 SHA256 一致，仅证明所读包摘要匹配。'
    Wait-QwenText '已读取字节：78587775'
    Assert-QwenHash $RealQoderArchive $expected
    if ((Get-Item -LiteralPath $RealQoderArchive).LastWriteTimeUtc.Ticks -ne $mtime -or @(Get-ChildItem -LiteralPath $parent -Force).Count -ne 1) { throw 'Read-only verification modified the official fixture' }
    Save-QwenEvidence 'qoder-real-archive-success'
    # Changing the target must invalidate success before another request is sent.
    Select-QoderTarget 'linux-amd64'
    Assert-QoderSuccessCleared
    Select-QoderTarget 'linux-amd64-baseline'
    Click-Qwen '只读校验 Qoder 归档'
    Wait-QwenText '归档与固定官方 SHA256 一致，仅证明所读包摘要匹配。'
    Wait-QwenText '已读取字节：78587775'
    # A separate same-basename copy with one changed byte is a real negative
    # control: success must not carry across a changed path or be accepted by
    # size/selected target alone. Same-path content mutation is not tested here.
    $damagedDir = Join-Path $homeDir 'Qoder 单字节损坏 归档'
    New-Item -ItemType Directory -Path $damagedDir | Out-Null
    $damaged = Join-Path $damagedDir 'qodercli-linux-x64-baseline.tar.gz'
    [IO.File]::Copy($RealQoderArchive, $damaged, $false)
    $stream = [IO.File]::Open($damaged, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try { $first = $stream.ReadByte(); $null = $stream.Seek(0, [IO.SeekOrigin]::Begin); $stream.WriteByte([byte]($first -bxor 1)); $stream.Flush($true) } finally { $stream.Dispose() }
    $damagedHash = (Get-FileHash -LiteralPath $damaged).Hash
    if ($damagedHash -eq $expected -or (Get-Item -LiteralPath $damaged).Length -ne 78587775) { throw 'Corruption negative-control setup failed' }
    $damagedMtime = (Get-Item -LiteralPath $damaged).LastWriteTimeUtc.Ticks
    Set-QwenPath 'Qoder 官方归档绝对路径' $damaged
    Assert-QoderSuccessCleared
    Click-Qwen '只读校验 Qoder 归档'
    Wait-QwenText '未能确认 Qoder 分发计划或包摘要。请检查目标、官方文件名、文件权限和支持的读取平台；没有获得校验成功结论。'
    Assert-QoderSuccessCleared
    Assert-QwenHash $damaged $damagedHash
    Assert-QwenHash $RealQoderArchive $expected
    if ((Get-Item -LiteralPath $damaged).LastWriteTimeUtc.Ticks -ne $damagedMtime -or @(Get-ChildItem -LiteralPath $damagedDir -Force).Count -ne 1 -or
        (Get-Item -LiteralPath $RealQoderArchive).LastWriteTimeUtc.Ticks -ne $mtime -or @(Get-ChildItem -LiteralPath $parent -Force).Count -ne 1) { throw 'UI verification changed archive bytes, metadata or directory contents' }
    Save-QwenEvidence 'qoder-one-byte-corruption-rejected'
    Record 'qoder-real-archive-readonly-ui' @{ target = 'linux-amd64-baseline'; version = '1.1.85'; bytes = 78587775; sha256 = $expected; targetChangeInvalidated = $true; pathChangeInvalidated = $true; oneByteCorruptionRejected = $true; unchangedFiles = $true; extracted = $false; executed = $false; windowsRuntimeSupportNotTested = $true }
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
    # Existing synthetic directory ensures this is real unsupported-host handling,
    # not a missing-path failure or a fabricated Linux success result.
    $cacheMtime = (Get-Item -LiteralPath $fixture).LastWriteTimeUtc.Ticks
    Set-QwenPath 'Qoder 缓存绝对目录' $fixture
    Click-Qwen '只读诊断 Qoder 缓存'
    Wait-QwenText '当前主机不支持 Qoder 缓存诊断；仅支持 Linux x86_64，没有读取缓存或获得清理授权。'
    Assert-QwenHash $archive $before
    if ((Get-Item -LiteralPath $fixture).LastWriteTimeUtc.Ticks -ne $cacheMtime -or @(Get-ChildItem -LiteralPath $fixture -Force).Count -ne 1) { throw 'Unsupported cache observation modified the fixture' }
    Save-QwenEvidence 'qoder-cache-unsupported-host'
    Record 'qoder-cache-unsupported-host' @{ existingDirectory = $true; unsupportedHost = $true; unchangedFiles = $true; linuxGuiSuccessNotTested = $true }
    Check-QoderRealArchive
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
    Record 'qoder-readonly-ui' @{ explicitPlan = $true; syntheticMismatchRejected = $true; filesUnchanged = $true; realArchiveSuccessTested = $true }
    Stop-App
    } catch {
        # Preserve the actual failure state before the installer runner stops the
        # app. The shared evidence writer checks names AND values for the canary.
        try { Save-QwenEvidence 'qoder-failure' } catch { Record 'qoder-failure-evidence-unavailable' 'Could not safely capture accessibility evidence' }
        throw
    }
}
