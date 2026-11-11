# Additive installed Windows UI acceptance. Only authored fixture files receive
# test DACLs. No official Agent runtime and no product-only testing hooks.
function Get-ConfigurationDacl([string]$path) {
    $acl = Get-Acl -LiteralPath $path
    return $acl.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access)
}
function Get-InactiveConfigurationSnapshot($fixture, [string]$manifest) {
    $files = Invoke-ConfigurationFixture @('snapshot-inactive', '--manifest', $manifest) | ConvertFrom-Json
    $acls = @($fixture.inactiveTargets | ForEach-Object { (Get-Acl -LiteralPath $_).Sddl })
    return ConvertTo-Json -InputObject @{ files = $files; acls = $acls } -Depth 5 -Compress
}
function Wait-ConfigurationSuccess {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $text = Get-InstallAccessibleText
        if ($text.Contains('配置失败（可能已写入部分配置，请检查备份与当前配置）:')) { throw 'Configuration unexpectedly failed' }
        if ($text.Contains('默认 profile 配置操作已完成（写前备份）。')) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Configuration success not observed through installed UI'
}
function Check-ConfigurationDaclPreservation([string]$manifest) {
    Stop-App
    $stage = 'configuration-success-dacl-preservation'
    $result = @{ passed = $false; agents = @(); officialAgentExecuted = $false; rollbackExercised = $false }
    try {
        $null = Invoke-ConfigurationFixture @('prepare-dacl', '--manifest', $manifest)
        $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
        $inactiveBaseline = Get-InactiveConfigurationSnapshot $fixture $manifest
        $owner = [Security.Principal.WindowsIdentity]::GetCurrent().User
        foreach ($agent in @(@{ id = 'claude'; name = 'Claude Code'; file = '.claude/CLAUDE.md'; protected = $true }, @{ id = 'gemini'; name = 'Gemini CLI'; file = '.gemini/GEMINI.md'; protected = $false })) {
            $target = Join-Path $fixture.home $agent.file
            # A protected current-user-only descriptor differs from the inherited
            # parent descriptor. Build it from scratch, not by preserving grants.
            if ($agent.protected) {
                $acl = [Security.AccessControl.FileSecurity]::new()
                $acl.SetAccessRuleProtection($true, $false)
                $rule = [Security.AccessControl.FileSystemAccessRule]::new($owner, [Security.AccessControl.FileSystemRights]::FullControl, [Security.AccessControl.AccessControlType]::Allow)
                $acl.AddAccessRule($rule)
                Set-Acl -LiteralPath $target -AclObject $acl
            }
            $beforeAcl = Get-Acl -LiteralPath $target
            $before = Get-ConfigurationDacl $target
            $beforeOwner = $beforeAcl.GetOwner([Security.Principal.SecurityIdentifier]).Value
            $beforeGroup = $beforeAcl.GetGroup([Security.Principal.SecurityIdentifier]).Value
            $parent = Get-Acl -LiteralPath (Split-Path $target)
            $otherParentGrants = @($parent.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]) | Where-Object { $_.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow -and $_.IdentityReference -ne $owner })
            if ($agent.protected) {
                $rules = @($beforeAcl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]))
                if (-not $beforeAcl.AreAccessRulesProtected -or $rules.Count -ne 1 -or $rules[0].IdentityReference -ne $owner -or $otherParentGrants.Count -eq 0 -or $before -ceq (Get-ConfigurationDacl (Split-Path $target))) { throw 'Fixture does not contrast restrictive file DACL with broader parent' }
            } elseif ($beforeAcl.AreAccessRulesProtected -or @($beforeAcl.GetAccessRules($false, $true, [Security.Principal.SecurityIdentifier])).Count -eq 0) { throw 'Ordinary inherited DACL fixture missing' }
            Check-Gui ("configuration-dacl-" + $agent.id + '-start') -KeepOpen
            Set-QwenPath '搜索 Agent' $agent.name
            $null = Wait-QwenControl ("应用默认配置 " + $agent.name) ([Windows.Automation.ControlType]::Button)
            if ((Get-ConfigurationDacl $target) -cne $before) { throw 'Discovery changed target DACL' }
            Click-Qwen ("应用默认配置 " + $agent.name)
            Confirm-ConfigurationOnce $agent.name
            Wait-ConfigurationSuccess
            $null = Wait-QwenControl ("应用默认配置 " + $agent.name) ([Windows.Automation.ControlType]::Button)
            $verified = Invoke-ConfigurationFixture @('verify-success', '--manifest', $manifest, '--agent', $agent.id) | ConvertFrom-Json
            if ((Get-ConfigurationDacl $target) -cne $before -or (Get-ConfigurationDacl $verified.backup) -cne $before) { throw 'Successful configuration or backup changed the original DACL' }
            foreach ($checkedPath in @($target, $verified.backup)) {
                $afterAcl = Get-Acl -LiteralPath $checkedPath
                if ($afterAcl.AreAccessRulesProtected -ne $beforeAcl.AreAccessRulesProtected -or $afterAcl.GetOwner([Security.Principal.SecurityIdentifier]).Value -cne $beforeOwner -or $afterAcl.GetGroup([Security.Principal.SecurityIdentifier]).Value -cne $beforeGroup) { throw 'DACL protection state, owner or primary group changed' }
            }
            if ((Get-InactiveConfigurationSnapshot $fixture $manifest) -cne $inactiveBaseline) { throw 'Successful configuration changed inactive targets or previous aliases' }
            Save-QwenEvidence ("configuration-dacl-" + $agent.id)
            Stop-App
            $null = Invoke-ConfigurationFixture @('verify-success', '--manifest', $manifest, '--agent', $agent.id)
            if ((Get-ConfigurationDacl $target) -cne $before -or (Get-ConfigurationDacl $verified.backup) -cne $before -or (Get-InactiveConfigurationSnapshot $fixture $manifest) -cne $inactiveBaseline) { throw 'Configuration evidence changed while closing application' }
            foreach ($checkedPath in @($target, $verified.backup)) {
                $closedAcl = Get-Acl -LiteralPath $checkedPath
                if ($closedAcl.AreAccessRulesProtected -ne $beforeAcl.AreAccessRulesProtected -or $closedAcl.GetOwner([Security.Principal.SecurityIdentifier]).Value -cne $beforeOwner -or $closedAcl.GetGroup([Security.Principal.SecurityIdentifier]).Value -cne $beforeGroup) { throw 'DACL protection state, owner or primary group changed while closing application' }
            }
            $entry = @{ agent = $agent.id; confirmations = 1; successfulManagedContent = $true; daclPreserved = $true; protectionPreserved = $true; protectedFixture = $agent.protected; ownerPreserved = $true; primaryGroupPreserved = $true; backupOriginalBytesAndDaclPreserved = $true; inactiveSettingsAndPreviousAliasesPreserved = $true; versionOnlyProbes = $verified.versionOnlyProbes; officialAgentExecuted = $false }
            $result.agents += $entry
            Record ("configuration-dacl-" + $agent.id) $entry
        }
        if ($result.agents.Count -ne 2) { throw 'Missing successful configuration DACL stage' }
        $result.passed = $true
        Record $stage $result
        return $result
    } finally {
        # No paths, SIDs, DACL strings, hashes or contents leave the private fixture.
        $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "$stage.json") -Encoding utf8
        Stop-App
    }
}
