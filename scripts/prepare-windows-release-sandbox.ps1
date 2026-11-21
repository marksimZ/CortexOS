param(
    [Parameter(Mandatory)][string]$CandidateDirectory,
    [Parameter(Mandatory)][string]$PublishedInstaller,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$output = [IO.Path]::GetFullPath($OutputDirectory)
$artifactRoot = [IO.Path]::GetFullPath((Join-Path $root 'artifacts')) + [IO.Path]::DirectorySeparatorChar
if (-not $output.StartsWith($artifactRoot, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $output)) {
    throw '选择 artifacts 下未使用的输出目录。'
}
$candidate = (Resolve-Path -LiteralPath $CandidateDirectory).Path
$metadata = Get-Content -Raw -LiteralPath (Join-Path $candidate 'candidate.json') | ConvertFrom-Json
$source = Join-Path $candidate $metadata.filename
$old = (Resolve-Path -LiteralPath $PublishedInstaller).Path
if ((Get-FileHash -LiteralPath $source).Hash -ne $metadata.sha256 -or
    (Get-FileHash -LiteralPath $old).Hash -ne 'B3A18E39B590C2615B024293E172E434949B263DAF37CD7D122FF8C71A9ED1EB') {
    throw '新旧安装包摘要校验不一致。'
}

$input = (New-Item -Type Directory -Path (Join-Path $output 'input')).FullName
$evidence = (New-Item -Type Directory -Path (Join-Path $output 'evidence')).FullName
Copy-Item -LiteralPath $source,(Join-Path $candidate 'candidate.json'),(Join-Path $PSScriptRoot 'windows-release-guest.mjs') -Destination $input
Copy-Item -LiteralPath $old -Destination (Join-Path $input 'CortexOS_0.2.0_x64-setup.exe')
$node = Get-Command node.exe -CommandType Application | Select-Object -First 1 -ExpandProperty Source
& $node -e 'require("node:sqlite")'
if ($LASTEXITCODE -ne 0) { throw '准备 Node 22.13+ x64（需提供内置 SQLite）。' }
Copy-Item -LiteralPath $node -Destination (Join-Path $input 'node.exe')

foreach ($mode in @('fresh', 'upgrade')) {
    $caseResult = (New-Item -Type Directory -Path (Join-Path $evidence $mode)).FullName
    $guestInput = [Security.SecurityElement]::Escape($input)
    $guestOutput = [Security.SecurityElement]::Escape($caseResult)
    $sandbox = @"
<Configuration>
  <VGpu>Disable</VGpu>
  <Networking>Disable</Networking>
  <AudioInput>Disable</AudioInput>
  <VideoInput>Disable</VideoInput>
  <PrinterRedirection>Disable</PrinterRedirection>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <MappedFolders>
    <MappedFolder><HostFolder>$guestInput</HostFolder><SandboxFolder>C:\CortexOS-input</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$guestOutput</HostFolder><SandboxFolder>C:\CortexOS-output</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>cmd.exe /D /C C:\CortexOS-input\node.exe C:\CortexOS-input\windows-release-guest.mjs $mode &gt; C:\CortexOS-output\guest.log 2&gt;&amp;1</Command>
  </LogonCommand>
</Configuration>
"@
    $path = Join-Path $output "$mode.wsb"
    [IO.File]::WriteAllText($path, $sandbox, [Text.UTF8Encoding]::new($false))
    Write-Output $path
}
