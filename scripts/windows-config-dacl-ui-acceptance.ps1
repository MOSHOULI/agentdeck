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
# Never publish raw UI errors: they may contain paths or configuration contents.
function Get-ConfigurationFailureCategory([string]$text) {
    $prefix = '配置失败（可能已写入部分配置，请检查备份与当前配置）:'
    $notices = @($text -split "`n" | Where-Object { $_.Contains($prefix) } | ForEach-Object { $_.Substring($_.IndexOf($prefix)) })
    if ($notices.Count -eq 0) { return 'no-known-configuration-error' }
    $notice = $notices -join "`n"
    $known = [ordered]@{
        'staging-owner-group' = 'staging owner/group differs from original'
        'staging-dacl-application' = 'staging DACL application failed'
        'staging-dacl-verification-acl-and-control' = 'staging DACL verification failed (ACL bytes and control flags)'
        'staging-dacl-verification-acl-bytes' = 'staging DACL verification failed (ACL bytes)'
        'staging-dacl-verification-control-flags' = 'staging DACL verification failed (control flags)'
        'staging-dacl-verification-owner-group' = 'staging DACL verification failed (owner/group)'
        'staging-dacl-verification' = 'staging DACL verification failed'
        'descriptor-inspection' = 'security descriptor inspection failed'
        'unsupported-dacl' = 'absent, null or defaulted DACL is unsupported'
        'invalid-dacl' = 'invalid DACL'
        'unsupported-ace-policy' = 'ordinary allow/deny subset'
        'owner-group-inspection' = 'owner/group inspection failed'
        'protection-inspection' = 'DACL protection inspection failed'
        'handle-security-drift' = 'configuration handle security changed'
        'snapshot-security-drift' = 'configuration security changed since snapshot'
    }
    foreach ($entry in $known.GetEnumerator()) { if ($notice.Contains($entry.Value)) { return $entry.Key } }
    if ($notice.Contains('Windows configuration security cannot be preserved:')) { return 'other-windows-security' }
    return 'other-configuration-error'
}
function Get-ConfigurationFailureDiagnostic($agent, $target, $beforeAcl, $before, $beforeOwner, $beforeGroup, $manifest) {
    $diagnostic = @{ category = 'ui-unavailable'; aclInspection = $false; bytesInspection = $false }
    try { $diagnostic.category = Get-ConfigurationFailureCategory (Get-InstallAccessibleText) } catch { }
    try {
        $current = Get-Acl -LiteralPath $target
        $oldRaw = [Security.AccessControl.RawSecurityDescriptor]::new($beforeAcl.GetSecurityDescriptorBinaryForm(), 0)
        $newRaw = [Security.AccessControl.RawSecurityDescriptor]::new($current.GetSecurityDescriptorBinaryForm(), 0)
        $diagnostic.aclInspection = $true
        $diagnostic.daclMatches = (Get-ConfigurationDacl $target) -ceq $before
        $diagnostic.protectionMatches = $current.AreAccessRulesProtected -eq $beforeAcl.AreAccessRulesProtected
        $diagnostic.ownerMatches = $current.GetOwner([Security.Principal.SecurityIdentifier]).Value -ceq $beforeOwner
        $diagnostic.groupMatches = $current.GetGroup([Security.Principal.SecurityIdentifier]).Value -ceq $beforeGroup
        $diagnostic.beforeProtected = $beforeAcl.AreAccessRulesProtected
        $diagnostic.beforeAceCount = $oldRaw.DiscretionaryAcl.Count
        $diagnostic.beforeAclBytes = $oldRaw.DiscretionaryAcl.BinaryLength
        $diagnostic.currentAceCountMatches = $newRaw.DiscretionaryAcl.Count -eq $oldRaw.DiscretionaryAcl.Count
        $diagnostic.currentAclBytesMatch = $newRaw.DiscretionaryAcl.BinaryLength -eq $oldRaw.DiscretionaryAcl.BinaryLength
        $diagnostic.beforeAutoInherited = ($oldRaw.ControlFlags -band [Security.AccessControl.ControlFlags]::DiscretionaryAclAutoInherited) -ne 0
        $diagnostic.beforeAutoInheritRequired = ($oldRaw.ControlFlags -band [Security.AccessControl.ControlFlags]::DiscretionaryAclAutoInheritRequired) -ne 0
        $diagnostic.daclControlMatches = (($oldRaw.ControlFlags -band 0x1500) -eq ($newRaw.ControlFlags -band 0x1500))
    } catch { }
    try {
        $state = Invoke-ConfigurationFixture @('diagnose-success', '--manifest', $manifest, '--agent', $agent.id) | ConvertFrom-Json
        $diagnostic.byteState = $state
        $diagnostic.bytesInspection = $true
    } catch { }
    return $diagnostic
}
function Check-ConfigurationDaclPreservation([string]$manifest) {
    Stop-App
    $stage = 'configuration-success-dacl-preservation'
    $result = @{ passed = $false; agents = @(); officialAgentExecuted = $false; rollbackExercised = $false }
    $agent = $null; $target = $null; $beforeAcl = $null; $before = $null
    $beforeOwner = $null; $beforeGroup = $null
    try {
        $null = Invoke-ConfigurationFixture @('prepare-dacl', '--manifest', $manifest)
        $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
        $owner = [Security.Principal.WindowsIdentity]::GetCurrent().User
        $agents = @(@{ id = 'claude'; name = 'Claude Code'; file = '.claude/CLAUDE.md'; protected = $true }, @{ id = 'gemini'; name = 'Gemini CLI'; file = '.gemini/GEMINI.md'; protected = $false })
        # Author every fixture ACL before taking the inactive baseline. Parent
        # inheritance can legitimately update the synthetic settings file here.
        foreach ($agent in $agents) {
            $target = Join-Path $fixture.home $agent.file
            # A protected current-user-only descriptor differs from the inherited
            # parent descriptor. Build it from scratch, not by preserving grants.
            if ($agent.protected) {
                $acl = [Security.AccessControl.FileSecurity]::new()
                $acl.SetAccessRuleProtection($true, $false)
                $rule = [Security.AccessControl.FileSystemAccessRule]::new($owner, [Security.AccessControl.FileSystemRights]::FullControl, [Security.AccessControl.AccessControlType]::Allow)
                $acl.AddAccessRule($rule)
                Set-Acl -LiteralPath $target -AclObject $acl
            } else {
                # A temporary directory's default DACL does not guarantee AI or
                # inherited ACEs. Establish a genuine private inheritable parent.
                $parentAcl = [Security.AccessControl.DirectorySecurity]::new()
                $parentAcl.SetAccessRuleProtection($true, $false)
                $inherit = [Security.AccessControl.InheritanceFlags]::ObjectInherit -bor [Security.AccessControl.InheritanceFlags]::ContainerInherit
                $parentRule = [Security.AccessControl.FileSystemAccessRule]::new($owner, [Security.AccessControl.FileSystemRights]::FullControl, $inherit, [Security.AccessControl.PropagationFlags]::None, [Security.AccessControl.AccessControlType]::Allow)
                $parentAcl.AddAccessRule($parentRule)
                Set-Acl -LiteralPath (Split-Path $target) -AclObject $parentAcl
                $childAcl = [Security.AccessControl.FileSecurity]::new()
                $childAcl.SetAccessRuleProtection($false, $false)
                Set-Acl -LiteralPath $target -AclObject $childAcl
            }
        }
        $inactiveBaseline = Get-InactiveConfigurationSnapshot $fixture $manifest
        foreach ($agent in $agents) {
            $target = Join-Path $fixture.home $agent.file
            $beforeAcl = Get-Acl -LiteralPath $target
            $before = Get-ConfigurationDacl $target
            $beforeOwner = $beforeAcl.GetOwner([Security.Principal.SecurityIdentifier]).Value
            $beforeGroup = $beforeAcl.GetGroup([Security.Principal.SecurityIdentifier]).Value
            $sourceDescriptor = [Security.AccessControl.RawSecurityDescriptor]::new($beforeAcl.GetSecurityDescriptorBinaryForm(), 0)
            $sourceAutoInherited = ($sourceDescriptor.ControlFlags -band [Security.AccessControl.ControlFlags]::DiscretionaryAclAutoInherited) -ne 0
            $sourceAutoInheritRequired = ($sourceDescriptor.ControlFlags -band [Security.AccessControl.ControlFlags]::DiscretionaryAclAutoInheritRequired) -ne 0
            if (-not $sourceAutoInherited -or $sourceAutoInheritRequired) { throw 'Expected native auto-inherited DACL control shape missing' }

            $parent = Get-Acl -LiteralPath (Split-Path $target)
            $otherParentGrants = @($parent.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]) | Where-Object { $_.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow -and $_.IdentityReference -ne $owner })
            if ($agent.protected) {
                $rules = @($beforeAcl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]))
                if (-not $beforeAcl.AreAccessRulesProtected -or $rules.Count -ne 1 -or $rules[0].IdentityReference -ne $owner -or $otherParentGrants.Count -eq 0 -or $before -ceq (Get-ConfigurationDacl (Split-Path $target))) { throw 'Fixture does not contrast restrictive file DACL with broader parent' }
            } elseif ($beforeAcl.AreAccessRulesProtected -or @($beforeAcl.GetAccessRules($false, $true, [Security.Principal.SecurityIdentifier])).Count -eq 0 -or @($beforeAcl.GetAccessRules($true, $false, [Security.Principal.SecurityIdentifier])).Count -ne 0) { throw 'Ordinary inherited DACL fixture missing' }
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
            $entry = @{ agent = $agent.id; confirmations = 1; successfulManagedContent = $true; daclPreserved = $true; protectionPreserved = $true; protectedFixture = $agent.protected; sourceAutoInherited = $sourceAutoInherited; sourceAutoInheritRequired = $sourceAutoInheritRequired; ownerPreserved = $true; primaryGroupPreserved = $true; backupOriginalBytesAndDaclPreserved = $true; inactiveSettingsAndPreviousAliasesPreserved = $true; versionOnlyProbes = $verified.versionOnlyProbes; officialAgentExecuted = $false }
            $result.agents += $entry
            Record ("configuration-dacl-" + $agent.id) $entry
        }
        if ($result.agents.Count -ne 2) { throw 'Missing successful configuration DACL stage' }
        $result.passed = $true
        Record $stage $result
        return $result
    } catch {
        # Capture while the failed UI still exists. Keep the original failure and
        # failed gate; diagnostic collection does not retry or permit success.
        $originalFailure = $_
        try { $result.failure = Get-ConfigurationFailureDiagnostic $agent $target $beforeAcl $before $beforeOwner $beforeGroup $manifest } catch { $result.failure = @{ category = 'diagnostic-collection-failed' } }
        throw $originalFailure
    } finally {
        # No paths, SIDs, DACL strings, hashes or contents leave the private fixture.
        $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "$stage.json") -Encoding utf8
        Stop-App
    }
}
