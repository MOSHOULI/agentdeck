# Actual PowerShell/Python UTF-8 roundtrip. Creates only private synthetic fixtures.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-install-ui-acceptance.ps1')
$script:acceptancePython = (Get-Command python.exe -CommandType Application | Select-Object -First 1).Source
$node = (Get-Command node.exe -CommandType Application | Select-Object -First 1).Source
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('安装 传输 ' + [guid]::NewGuid().ToString('N'))
$testHome = Join-Path $testRoot '用户 数据'
$fixture = Join-Path $testRoot '管理器 测试'
$originalConsole = [Console]::OutputEncoding
$originalOutput = $OutputEncoding
try {
    New-Item -ItemType Directory -Path $testHome -Force | Out-Null
    # Deliberately use a non-Unicode console outside the production wrapper.
    [Console]::OutputEncoding = [Text.Encoding]::GetEncoding(1252)
    $OutputEncoding = [Text.Encoding]::GetEncoding(1252)
    $manifest = Invoke-InstallFixture @('--prepare', $fixture, '--node', $node, '--home', $testHome)
    if (@($manifest).Count -ne 1 -or -not (Test-Path -LiteralPath $manifest -PathType Leaf)) { throw 'UTF8 fixture output did not resolve' }
    if (-not $manifest.Contains('管理器 测试')) { throw 'UTF8 path roundtrip corrupted' }
    if ([Console]::OutputEncoding.CodePage -ne 1252 -or $OutputEncoding.CodePage -ne 1252) { throw 'Fixture wrapper changed caller encoding' }
    $counts = Invoke-InstallFixture @('--assert', 'absent', '--manifest', $manifest) | ConvertFrom-Json
    if ($counts.versionProbes -ne 0) { throw 'Transport regression unexpectedly executed synthetic agent' }
    Write-Output 'Windows fixture UTF8 transport regression passed.'
} finally {
    [Console]::OutputEncoding = $originalConsole
    $OutputEncoding = $originalOutput
    if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
}
