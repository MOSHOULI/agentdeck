# Run only on a disposable GitHub-hosted Windows runner. Never on a user PC.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$BaselineInstaller,
    [Parameter(Mandatory)][string]$ExpectedVersion,
    [Parameter(Mandatory)][string]$ExpectedCommit,
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [Parameter(Mandatory)][string]$RealQoderArchive
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'This destructive lifecycle test requires a disposable GitHub-hosted Windows runner.'
}
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$BaselineInstaller = (Resolve-Path -LiteralPath $BaselineInstaller).Path
$RealQoderArchive = (Resolve-Path -LiteralPath $RealQoderArchive).Path
$EvidenceDirectory = [IO.Path]::GetFullPath($EvidenceDirectory)
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
$root = Join-Path $env:RUNNER_TEMP ('AgentDeck 验收 ' + [guid]::NewGuid().ToString('N'))
$installDir = Join-Path $root '程序 空格'
$homeDir = Join-Path $root '用户 数据'
New-Item -ItemType Directory -Path $homeDir -Force | Out-Null
$events = [Collections.Generic.List[object]]::new()
$running = $null
$passed = $false
$script:acceptanceNode = (Get-Command node.exe -CommandType Application | Select-Object -First 1).Source
$script:acceptanceGit = (Get-Command git.exe -CommandType Application | Select-Object -First 1).Source
$script:acceptancePython = (Get-Command python.exe -CommandType Application | Select-Object -First 1).Source
$originalEnv = @{}
$names = @('HOME','USERPROFILE','APPDATA','LOCALAPPDATA','AGENTDECK_TEST_HOME','PATH','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
foreach ($name in $names) { $originalEnv[$name] = [Environment]::GetEnvironmentVariable($name) }
function Record([string]$name, [object]$details) {
    $events.Add(@{ stage = $name; utc = [DateTime]::UtcNow.ToString('o'); details = $details })
}
function Run-Installer([string]$path) {
    # NSIS /D must be last and unquoted, even when it includes spaces.
    $p = Start-Process -FilePath $path -ArgumentList "/S /D=$installDir" -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "Installer returned $($p.ExitCode)" }
    if (-not (Test-Path -LiteralPath (Join-Path $installDir 'agentdeck.exe'))) { throw 'Installed executable missing' }
}
function Assert-Version {
    $version = (Get-Item -LiteralPath (Join-Path $installDir 'agentdeck.exe')).VersionInfo.ProductVersion
    if ($version -ne $ExpectedVersion) { throw "Wrong installed version: $version, expected $ExpectedVersion" }
    Record 'installed-version' $version
}
function Stop-App {
    if ($null -ne $script:running) {
        if (-not $script:running.HasExited) {
            $null = $script:running.CloseMainWindow()
            if (-not $script:running.WaitForExit(15000)) {
                Stop-Process -Id $script:running.Id -Force
                throw 'Application did not exit after window close'
            }
        }
        $script:running.Dispose()
        $script:running = $null
    }
}
function Uninstall-App {
    $uninstaller = Join-Path $installDir 'uninstall.exe'
    if (-not (Test-Path -LiteralPath $uninstaller)) { throw 'Uninstaller missing' }
    # _?= avoids NSIS copying the uninstaller then returning before it finishes.
    $p = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$installDir" -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "Uninstaller returned $($p.ExitCode)" }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ((Test-Path -LiteralPath (Join-Path $installDir 'agentdeck.exe')) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    if (Test-Path -LiteralPath (Join-Path $installDir 'agentdeck.exe')) { throw 'Uninstall left the application executable' }
}
function Assert-Sentinel {
    if (-not (Test-Path -LiteralPath $script:sentinel) -or [IO.File]::ReadAllText($script:sentinel) -ne $script:sentinelValue) {
        throw 'Synthetic user data was removed or changed'
    }
}
function Check-Gui([string]$stage, [switch]$KeepOpen) {
    $script:running = Start-Process -FilePath (Join-Path $installDir 'agentdeck.exe') -WorkingDirectory $homeDir -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    $found = $false
    $lastNames = @()
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($script:running.HasExited) { throw "Application exited before GUI acceptance: $($script:running.ExitCode)" }
        try {
        $condition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $script:running.Id)
        $window = [Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children, $condition)
        if ($null -ne $window) {
            $nodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
            Assert-AcceptanceCanariesAbsent $nodes
            $lastNames = @($nodes | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
            # Native mode proves this is the Tauri WebView, not the browser demo.
            # The card only renders after get_agents resolves through the real IPC.
            if (($lastNames -contains '本地桌面模式') -and ($lastNames -contains 'Claude Code') -and ($lastNames -contains 'Agent 管理')) {
                $found = $true
                break
            }
        }
        } catch [Windows.Automation.ElementNotAvailableException] {
            # Startup can replace the WebView accessibility subtree. Retry only this transient error.
        }
        Start-Sleep -Milliseconds 500
    }
    # Last observed nodes were checked before accepting or persisting their names.
    $lastNames | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "$stage-accessibility.json") -Encoding utf8
    if (-not $found) { throw "Native WebView content/IPC did not become accessible ($stage)" }
    $currentNodes = $window.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    Assert-AcceptanceCanariesAbsent $currentNodes
    # Evidence only: screen capture is not itself the acceptance assertion.
    try {
    Add-Type -AssemblyName System.Drawing, System.Windows.Forms
    $bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = [Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save((Join-Path $EvidenceDirectory "$stage.png"), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
    } catch { Record "$stage-screenshot-unavailable" $_.Exception.Message }
    Record $stage @{ nativeWebView = $true; agentListIpc = $true; processId = $script:running.Id }
    if (-not $KeepOpen) { Stop-App }
}
try {
    # Installer registration is in the disposable runner's HKCU. Application state
    # and Agent discovery use only this synthetic home and a minimal system PATH.
    $env:HOME = $homeDir
    $env:USERPROFILE = $homeDir
    $env:AGENTDECK_TEST_HOME = $homeDir
    $env:APPDATA = Join-Path $homeDir 'AppData\Roaming'
    $env:LOCALAPPDATA = Join-Path $homeDir 'AppData\Local'
    New-Item -ItemType Directory -Force -Path $env:APPDATA,$env:LOCALAPPDATA | Out-Null
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--force-renderer-accessibility'
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    . (Join-Path $PSScriptRoot 'windows-qwen-ui-acceptance.ps1')
    . (Join-Path $PSScriptRoot 'windows-qoder-ui-acceptance.ps1')
    . (Join-Path $PSScriptRoot 'windows-codex-ui-acceptance.ps1')
    . (Join-Path $PSScriptRoot 'windows-install-ui-acceptance.ps1')
    . (Join-Path $PSScriptRoot 'windows-memory-ui-acceptance.ps1')
    . (Join-Path $PSScriptRoot 'windows-config-failure-ui-acceptance.ps1')
    . (Join-Path $PSScriptRoot 'windows-config-dacl-ui-acceptance.ps1')
    $sentinel = Join-Path $homeDir '.agentdeck\acceptance-user-data.txt'
    $sentinelValue = 'Synthetic data, must survive installation, upgrade and uninstall.'
    New-Item -ItemType Directory -Force -Path (Split-Path $sentinel) | Out-Null
    [IO.File]::WriteAllText($sentinel, $sentinelValue)
    Record 'inputs' @{ commit = $ExpectedCommit; version = $ExpectedVersion; installerSha256 = (Get-FileHash $Installer).Hash; baselineSha256 = (Get-FileHash $BaselineInstaller).Hash }
    Run-Installer $Installer
    Assert-Version
    Assert-Sentinel
    Check-Gui 'clean-install' -KeepOpen
    Check-QwenLifecycle
    Check-QwenCurrentLifecycle
    Check-Gui 'restart' -KeepOpen
    Check-QoderReadonly
    Check-ReviewedInstall
    Check-FailedReviewedInstall
    Check-WrongRootReviewedInstall
    Check-CodexReadonly
    Check-MemoryExport
    Check-ConfigurationFailurePreservation
    Run-Installer $Installer
    Assert-Version
    Assert-Sentinel
    Check-Gui 'same-version-reinstall'
    Uninstall-App
    Assert-Sentinel
    Record 'clean-uninstall' @{ userDataPreserved = $true }
    Run-Installer $BaselineInstaller
    $oldVersion = (Get-Item -LiteralPath (Join-Path $installDir 'agentdeck.exe')).VersionInfo.ProductVersion
    if ($oldVersion -ne '1.1.1') { throw "Unexpected baseline version: $oldVersion" }
    Assert-Sentinel
    Run-Installer $Installer
    Assert-Version
    Assert-Sentinel
    Check-Gui 'upgrade-from-1.1.1'
    Uninstall-App
    Assert-Sentinel
    Record 'upgrade-uninstall' @{ userDataPreserved = $true }
    $passed = $true
} catch {
    Record 'failure' $_.Exception.Message
    throw
} finally {
    if ($null -ne $running -and -not $running.HasExited) { Stop-Process -Id $running.Id -Force -ErrorAction SilentlyContinue }
    # Preserve runner-local evidence on failure; never delete a real user's data.
    @{ passed = $passed; buildCommit = $ExpectedCommit; events = $events; limitations = @('Authored Claude/Gemini version-only fixture; configuration rejection tested, no official runtime or successful configuration acceptance', 'Synthetic settings plus a pinned official Linux archive (read only)', 'Authored memory SQLite and local Markdown UI acceptance only; no real sessions, collection or agent consumption verified', 'Qoder UI hashes a Linux archive on Windows; no Qoder execution or Windows runtime validation', 'Authored npm and authored Codex version fixture execute through the actual install adapter; no official Agent execution or authentication', 'Synthetic Codex Windows read-only UI observation only; no Windows writer or actual Codex runtime verification', 'Synthetic Qwen single-field UI lifecycle only; not real Qwen execution or complete GUI coverage', 'Published baseline is verified against its published checksum, not an independent signature') } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $originalEnv[$name]) }
}
