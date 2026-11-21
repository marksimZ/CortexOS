param([Parameter(Mandatory)][int]$AppProcessId)
$ErrorActionPreference = 'Stop'
$processes = Get-CimInstance -ClassName Win32_Process -Property ProcessId,ParentProcessId,Name,WorkingSetSize,PrivatePageCount,HandleCount,KernelModeTime,UserModeTime
$tree = [Collections.Generic.HashSet[int]]::new()
[void]$tree.Add($AppProcessId)
do {
  $previous = $tree.Count
  foreach ($process in $processes) {
    if ($tree.Contains($process.ParentProcessId)) { [void]$tree.Add($process.ProcessId) }
  }
} while ($tree.Count -gt $previous)
$rows = @($processes | Where-Object { $tree.Contains($_.ProcessId) } | ForEach-Object {
  @{
    pid = $_.ProcessId
    webview = $_.Name -eq 'msedgewebview2.exe'
    rss = $_.WorkingSetSize
    private = $_.PrivatePageCount
    handles = $_.HandleCount
    cpuTicks = [long]$_.KernelModeTime + [long]$_.UserModeTime
  }
})
$gpu = $null
foreach ($driverPath in @("$env:SystemRoot/System32/nvidia-smi.exe", "$env:ProgramW6432/NVIDIA Corporation/NVSMI/nvidia-smi.exe")) {
  if (Test-Path -LiteralPath $driverPath -PathType Leaf) {
    $gpu = @(& $driverPath '--query-gpu=memory.used,utilization.gpu' '--format=csv,noheader,nounits' 2>$null)
    if ($LASTEXITCODE -ne 0) { $gpu = $null }
    break
  }
}
@{ processes = $rows; wholeGpu = $gpu } | ConvertTo-Json -Depth 4 -Compress
