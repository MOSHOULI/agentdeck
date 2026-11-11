# Executes the production route controller with mocked native boundaries.
# No GUI input, app launch, filesystem mutation, or agent execution.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
. (Join-Path $PSScriptRoot 'windows-install-ui-acceptance.ps1')
function Wait-QwenControl($name, $type) { return $script:tab }
function Get-InstallWorkspaceInvoke($tab) {
    if ($script:case.route -eq 'invoke') { return $script:invoke }
    return $null
}
function Prepare-InstallWorkspaceKeyboard($tab) {
    $script:preparations++
    if ($script:case.fault -eq 'prepare') { throw 'modeled-prepare-failure' }
    if ($script:case.fault -eq 'zero-handle') { return [IntPtr]::Zero }
    return [IntPtr]123
}
function Get-InstallForegroundWindow {
    $script:foregroundReads++
    if ($script:foregroundReads -gt 1) {
        if ($script:case.fault -eq 'foreground') { return [IntPtr]456 }
        if ($script:case.fault -eq 'focus') { $script:tab.Current.HasKeyboardFocus = $false }
    }
    return [IntPtr]123
}
function Send-InstallWorkspaceEnterKey {
    $script:keys++
    if ($script:case.fault -eq 'send') { throw 'modeled-send-failure' }
}
function Wait-InstallWorkspaceSelected($name) {
    $script:waits++
    if ($script:case.fault -eq 'settle') { throw 'modeled-settle-failure' }
}
function Record($stage, $details) { $script:records.Add(@{ stage = $stage; details = $details }) }
function Save-QwenEvidence($stage) {
    $script:captures++
    if ($script:case.fault -eq 'invoke') { throw 'modeled-evidence-unavailable' }
}
$cases = @(
    @{ route = 'invoke'; fault = ''; invokes = 1; keys = 0; preparations = 0; waits = 1 },
    @{ route = 'invoke'; fault = 'invoke'; invokes = 1; keys = 0; preparations = 0; waits = 0 },
    @{ route = 'invoke'; fault = 'settle'; invokes = 1; keys = 0; preparations = 0; waits = 1 },
    @{ route = 'keyboard'; fault = ''; invokes = 0; keys = 1; preparations = 1; waits = 1 },
    @{ route = 'keyboard'; fault = 'prepare'; invokes = 0; keys = 0; preparations = 1; waits = 0 },
    @{ route = 'keyboard'; fault = 'foreground'; invokes = 0; keys = 0; preparations = 1; waits = 0 },
    @{ route = 'keyboard'; fault = 'focus'; invokes = 0; keys = 0; preparations = 1; waits = 0 },
    @{ route = 'keyboard'; fault = 'zero-handle'; invokes = 0; keys = 0; preparations = 1; waits = 0 },
    @{ route = 'keyboard'; fault = 'send'; invokes = 0; keys = 1; preparations = 1; waits = 0 },
    @{ route = 'keyboard'; fault = 'settle'; invokes = 0; keys = 1; preparations = 1; waits = 1 }
)
foreach ($script:case in $cases) {
    $script:invokes = 0; $script:keys = 0; $script:preparations = 0; $script:waits = 0
    $script:foregroundReads = 0; $script:captures = 0
    $script:records = [Collections.Generic.List[object]]::new()
    $script:tab = [pscustomobject]@{ Current = [pscustomobject]@{ HasKeyboardFocus = $true } }
    $script:invoke = [pscustomobject]@{}
    $script:invoke | Add-Member -MemberType ScriptMethod -Name Invoke -Value {
        $script:invokes++
        if ($script:case.fault -eq 'invoke') { throw 'modeled-invoke-failure' }
    }
    $failure = $null
    try { Activate-InstallWorkspace 'Qwen 配置检查' } catch { $failure = $_ }
    if (($null -ne $failure) -ne ($script:case.fault -ne '')) { throw 'Unexpected activation result' }
    foreach ($key in @('invokes','keys','preparations','waits')) {
        if ((Get-Variable -Scope Script -Name $key -ValueOnly) -ne $script:case[$key]) { throw "Activation route count mismatch: $key" }
    }
    if ($script:captures -ne [int]($script:case.fault -ne '')) { throw 'Failure evidence count mismatch' }
    if ($script:case.fault -eq 'invoke' -and -not $failure.Exception.Message.Contains('modeled-invoke-failure')) { throw 'Evidence failure masked original activation failure' }
    if ($script:case.fault -eq '' -and $script:case.route -eq 'keyboard') {
        $activation = @($script:records | Where-Object { $_.stage -eq 'installed-workspace-activation' })
        if ($activation.Count -ne 1 -or -not $activation[0].details.focusConfirmed -or -not $activation[0].details.foregroundConfirmed) { throw 'Keyboard evidence missing confirmed targets' }
    }
}
Write-Output 'Workspace activation route regressions passed (10 modeled cases; no native GUI input).'
