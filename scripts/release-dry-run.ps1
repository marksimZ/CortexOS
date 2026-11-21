param(
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+(?:-rc\.\d+)?$')][string]$Version,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$SevenZip,
    [switch]$RunTests
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$desktop = Join-Path $root 'apps/desktop'
$artifactRoot = [IO.Path]::GetFullPath((Join-Path $root 'artifacts')) + [IO.Path]::DirectorySeparatorChar
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (-not $output.StartsWith($artifactRoot, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $output)) {
    throw '选择 artifacts 下未使用的输出目录。'
}
$sevenZip = (Resolve-Path -LiteralPath $SevenZip).Path
$env:CARGO_NET_OFFLINE = 'true'

function Invoke-ReleasedTool($Tool, [string[]]$Arguments) {
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Tool 执行失败，退出码 $LASTEXITCODE。" }
}

$null = New-Item -ItemType Directory -Path $output
$web = Join-Path $output 'frontend'
$extracted = Join-Path $output 'extracted'
$tauriDir = Join-Path $desktop 'src-tauri'
$manifest = Join-Path $tauriDir 'Cargo.toml'
$tauriConfig = Get-Content -Raw -LiteralPath (Join-Path $tauriDir 'tauri.conf.json') | ConvertFrom-Json
$package = Get-Content -Raw -LiteralPath (Join-Path $desktop 'package.json') | ConvertFrom-Json
$packageLock = Get-Content -Raw -LiteralPath (Join-Path $desktop 'package-lock.json') | ConvertFrom-Json -AsHashtable
$cargoVersion = [regex]::Match((Get-Content -Raw -LiteralPath $manifest), '(?m)^version = "([^"\r\n]+)"$').Groups[1].Value
$baseVersions = @($tauriConfig.version, $package.version, $packageLock.version, $cargoVersion)
if ($tauriConfig.identifier -cne 'com.cortexos.desktop' -or $tauriConfig.productName -cne 'CortexOS' -or
    $tauriConfig.bundle.windows.nsis.installMode -cne 'currentUser' -or
    @($baseVersions | Select-Object -Unique).Count -ne 1) {
    throw '正式安装身份或基础版本配置不一致。'
}
if (-not ((& rustc -vV) -contains 'host: x86_64-pc-windows-msvc')) { throw '需要 Windows x64 MSVC 工具链。' }

Push-Location -LiteralPath $desktop
try {
    Invoke-ReleasedTool npm.cmd @('run', 'typecheck')
    Invoke-ReleasedTool cargo @('check', '--manifest-path', $manifest, '--offline', '--locked', '--features', 'tauri/custom-protocol')
    if ($RunTests) {
        Invoke-ReleasedTool npm.cmd @('run', 'test:unit')
        Invoke-ReleasedTool cargo @('test', '--manifest-path', $manifest, '--offline', '--locked', '--lib', 'storage::')
    }
    Invoke-ReleasedTool (Join-Path $desktop 'node_modules/.bin/vite.cmd') @('build', '--sourcemap', 'hidden', '--outDir', $web)
    $metadata = & cargo metadata --manifest-path $manifest --offline --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw '读取锁定的 Windows Rust 依赖失败。' }
    [IO.File]::WriteAllText((Join-Path $output 'cargo-windows.json'), [string]$metadata, [Text.UTF8Encoding]::new($false))
    Invoke-ReleasedTool py.exe @('-3', (Join-Path $PSScriptRoot 'release_inventory.py'), $output, $web)

    $mapOutput = (New-Item -ItemType Directory -Path (Join-Path $output 'source-maps')).FullName
    Get-ChildItem -LiteralPath $web -Recurse -File -Filter '*.map' | ForEach-Object {
        $name = $_.FullName
        if (-not $name.StartsWith($web + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw '源映射路径超出本次候选构建目录。'
        }
        Move-Item -LiteralPath $name -Destination (Join-Path $mapOutput $_.Name)
    }
    $notices = Join-Path $output 'THIRD-PARTY-NOTICES.txt'
    $materials = Join-Path $output 'third-party-materials.zip'
    $overlay = @{
        version = $Version
        build = @{ frontendDist = $web; beforeBuildCommand = $null }
        bundle = @{ createUpdaterArtifacts = $false; resources = @{
            "$notices" = 'THIRD-PARTY-NOTICES.txt'; "$materials" = 'third-party-materials.zip'
        } }
    }
    $overlayPath = Join-Path $output 'tauri-candidate.json'
    [IO.File]::WriteAllText($overlayPath, ($overlay | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $start = [DateTime]::UtcNow
    Invoke-ReleasedTool npm.cmd @('run', 'tauri', '--', 'build', '--ci', '--no-sign', '--bundles', 'nsis', '--config', $overlayPath, '--', '--offline', '--locked')
    $name = "CortexOS_${Version}_x64-setup.exe"
    $exe = Get-Item -LiteralPath (Join-Path $tauriDir "target/release/bundle/nsis/$name")
    if ($exe.LastWriteTimeUtc -lt $start) { throw '候选安装包不是本次构建产物。' }
    $candidate = Join-Path $output $name
    Copy-Item -LiteralPath $exe.FullName -Destination $candidate
    Invoke-ReleasedTool $sevenZip @('t', '-tNsis', $candidate)
    Invoke-ReleasedTool $sevenZip @('x', '-tNsis', $candidate, "-o$extracted", '-y')

    $payload = @(Get-ChildItem -LiteralPath $extracted -Recurse -File | ForEach-Object {
        $relative = [IO.Path]::GetRelativePath($extracted, $_.FullName).Replace('\', '/')
        if ($relative -notmatch '^(?:cortexos-desktop\.exe|uninstall\.exe|THIRD-PARTY-NOTICES\.txt|third-party-materials\.zip|\$PLUGINSDIR/(?:System\.dll|StartMenu\.dll|nsis_tauri_utils\.dll|NSISdl\.dll|nsDialogs\.dll|modern-wizard\.bmp|LangDLL\.dll))$') {
            throw "发现安装包未批准条目 $relative"
        }
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        [ordered]@{ path = $relative; bytes = $_.Length; sha256 = $hash }
    })
    foreach ($expected in @('cortexos-desktop.exe', 'uninstall.exe', 'THIRD-PARTY-NOTICES.txt', 'third-party-materials.zip')) {
        if ($expected -cnotin $payload.path) { throw "安装包缺少 $expected" }
    }
    $installerPlugin = Join-Path $extracted '$PLUGINSDIR/nsis_tauri_utils.dll'
    if ((Get-FileHash -Algorithm SHA1 -LiteralPath $installerPlugin).Hash -ne '75197FEE3C6A814FE035788D1C34EAD39349B860') {
        throw '安装组件与所附的 Tauri NSIS 插件版本声明不一致。'
    }
    foreach ($expected in @($notices, $materials)) {
        $installed = Join-Path $extracted ([IO.Path]::GetFileName($expected))
        if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $expected).Hash) { throw '随包声明材料校验失败。' }
    }
    $program = Join-Path $extracted 'cortexos-desktop.exe'
    Invoke-ReleasedTool py.exe @('-3', '-c', @'
import re, sys
from pathlib import Path
for name in sys.argv[1:]:
    data = Path(name).read_bytes()
    for expr in (rb'Gittoken\.txt', rb'-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----', rb'gh[pousr]_[a-zA-Z0-9]{36,}', rb'github_pat_[a-zA-Z0-9_]{60,}'):
        if re.search(expr, data, re.I):
            raise SystemExit('候选产物命中凭据扫描模式。')
'@, $candidate, $program)
    $hash = (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash
    $meta = [ordered]@{
        filename = $name; version = $Version; bytes = (Get-Item -LiteralPath $candidate).Length; sha256 = $hash
        productVersion = (Get-Item -LiteralPath $program).VersionInfo.ProductVersion
        installerAuthenticode = (Get-AuthenticodeSignature -LiteralPath $candidate).Status.ToString()
        programAuthenticode = (Get-AuthenticodeSignature -LiteralPath $program).Status.ToString()
        sourceCommit = (& git rev-parse HEAD); sourceModified = [bool](& git status --porcelain); payload = $payload; posted = $false
    }
    $meta | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'candidate.json') -Encoding utf8
    "$hash  $name" | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
    @"
# CortexOS $Version 候选包说明

- 可以备份和恢复本地工作区，保留知识版本、评测历史和应用设置。
- 设置中的本机诊断可查看后台任务与最近错误，并导出脱敏报告。
- 此候选使用手动更新。应用版本、校验值和签名状态见 candidate.json。
- 面向本次安装验收，正式分发以 h4-release-checklist.md 的批准项为准。
"@ | Set-Content -LiteralPath (Join-Path $output 'RELEASE-NOTES.md') -Encoding utf8
    Get-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt')
} finally {
    Pop-Location
}
