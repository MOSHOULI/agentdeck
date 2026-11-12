# Real Windows filesystem snapshot regression with authored npm only, no GUI/agent.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-install-ui-acceptance.ps1')
$script:acceptancePython = (Get-Command python.exe -CommandType Application | Select-Object -First 1).Source
$node = (Get-Command node.exe -CommandType Application | Select-Object -First 1).Source
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('失败 残留 ' + [guid]::NewGuid().ToString('N'))
$testHome = Join-Path $testRoot '用户 数据'
try {
    New-Item -ItemType Directory -Path $testHome -Force | Out-Null
    $manifest = Invoke-InstallFixture @('--prepare', (Join-Path $testRoot '工具 空格'), '--node', $node, '--home', $testHome, '--mode', 'fail')
    $fixture = Get-Content -LiteralPath $manifest -Raw -Encoding UTF8 | ConvertFrom-Json
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $fixture.node; $info.UseShellExecute = $false
    $info.Environment.Clear(); $info.Environment['SystemRoot'] = $env:SystemRoot
    foreach ($argument in @($fixture.manager,'install','--global','--prefix',$fixture.prefix,'--force=false','--no-audit','--no-fund','--no-progress','--ignore-scripts','@openai/codex@1.2.3')) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($info)
    try {
        if (-not $process.WaitForExit(10000)) { $process.Kill($true); throw 'Authored failure fixture timed out' }
        if ($process.ExitCode -ne 64) { throw 'Authored failure fixture did not fail as expected' }
    } finally { $process.Dispose() }
    $null = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest)
    # Reproduce the .NET enumeration cache independently of product operations.
    # Deliberately cache the parent before a known timestamp mutation, then
    # require Refresh() to obtain the changed value rather than ignoring mtime.
    $cachedParent = [IO.DirectoryInfo]::new($fixture.globalRoot)
    $cachedTime = $cachedParent.LastWriteTimeUtc
    $authoredTime = $cachedTime.AddMinutes(-5)
    [IO.Directory]::SetLastWriteTimeUtc($fixture.globalRoot, $authoredTime)
    if ($cachedParent.LastWriteTimeUtc -ne $cachedTime) { throw 'DirectoryInfo cache fixture did not retain its cached timestamp' }
    $cachedParent.Refresh()
    if ($cachedParent.LastWriteTimeUtc -ne $authoredTime) { throw 'DirectoryInfo Refresh did not obtain current metadata' }
    $cachedSnapshot = Get-FailedInstallState $fixture
    [IO.Directory]::SetLastWriteTimeUtc($fixture.globalRoot, $cachedTime)
    $delta = Get-FailedStateDifference $cachedSnapshot (Get-FailedInstallState $fixture)
    if (-not (@($delta | ForEach-Object { $_.fields }) -contains 'modified')) { throw 'Actual directory mtime mutation was not detected' }
    $before = Get-FailedInstallState $fixture
    $null = Invoke-InstallFixture @('--assert', 'failed', '--manifest', $manifest)
    $after = Get-FailedInstallState $fixture
    if ($before -cne $after) {
        Get-FailedStateDifference $before $after | ConvertTo-Json -Depth 5 -Compress | Write-Output
        throw 'Unchanged authored residue snapshot is unstable'
    }
    $partial = Join-Path $fixture.package 'partial-install.txt'
    [IO.File]::AppendAllText($partial, 'authored mutation')
    $delta = Get-FailedStateDifference $before (Get-FailedInstallState $fixture)
    if (-not (@($delta | ForEach-Object { $_.fields }) -contains 'hash')) { throw 'Byte mutation was not detected' }
    $current = Get-FailedInstallState $fixture
    $originalAttributes = [IO.File]::GetAttributes($partial)
    try {
        [IO.File]::SetAttributes($partial, ($originalAttributes -bxor [IO.FileAttributes]::Hidden))
        $delta = Get-FailedStateDifference $current (Get-FailedInstallState $fixture)
        if (-not (@($delta | ForEach-Object { $_.fields }) -contains 'attributes')) { throw 'Attribute mutation was not detected' }
    } finally { [IO.File]::SetAttributes($partial, $originalAttributes) }
    $current = Get-FailedInstallState $fixture
    [IO.File]::WriteAllText((Join-Path $fixture.package 'extra.txt'), 'authored extra')
    $delta = Get-FailedStateDifference $current (Get-FailedInstallState $fixture)
    if (-not (@($delta | ForEach-Object { $_.fields }) -contains 'entry-count')) { throw 'Extra residual entry was not detected' }
    Write-Output 'Authored failed-install snapshot and mutation regressions passed.'
} finally {
    if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
}
