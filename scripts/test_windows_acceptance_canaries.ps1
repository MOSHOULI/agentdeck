# Executable synthetic guard regressions. No application, IPC or external writes.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
. (Join-Path $PSScriptRoot 'windows-qwen-ui-acceptance.ps1')
function Assert-Rejected([scriptblock]$action, [string]$message) {
    $rejected = $false
    try { & $action } catch {
        if (-not $_.Exception.Message.Contains($message)) { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw 'Expected canary/provider failure was not rejected' }
}
Assert-AcceptanceCanaryValues $null $null
Assert-AcceptanceCanaryValues '' ''
Assert-AcceptanceCanaryValues 'ordinary name' $null
Assert-AcceptanceCanaryValues $null 'ordinary value'
foreach ($canary in @('SYNTHETIC-NOT-A-CREDENTIAL', 'AGENTDECK_SYNTHETIC_INSTALL_PRIVATE_CANARY')) {
    Assert-Rejected { Assert-AcceptanceCanaryValues ("prefix $canary suffix") $null } 'Synthetic canary leaked in accessibility tree'
    Assert-Rejected { Assert-AcceptanceCanaryValues $null ("prefix $canary suffix") } 'Synthetic canary leaked in accessible value'
}
# Exercise production node iteration with null Name and no ValuePattern.
$node = [pscustomobject]@{ Current = [pscustomobject]@{ Name = $null } }
$node | Add-Member ScriptMethod TryGetCurrentPattern { param($id, $pattern) return $false }
Assert-AcceptanceCanariesAbsent @($node)
# Provider errors remain fatal instead of becoming silent missing evidence.
$broken = [pscustomobject]@{ Current = [pscustomobject]@{ Name = $null } }
$broken | Add-Member ScriptMethod TryGetCurrentPattern { param($id, $pattern) throw 'authored-provider-error' }
Assert-Rejected { Assert-AcceptanceCanariesAbsent @($broken) } 'authored-provider-error'
Write-Output 'Synthetic null/name/value canary and provider-error regressions passed.'
