# smoke de recursos do app Mika: CPU% (deltas por PID) e working set agregado, incluindo os
# processos filhos do msedgewebview2 (o renderer do Tauri anda em processos próprios) e, se houver
# nvidia-smi, a VRAM do host antes vs em serviço.
# Uso: pwsh -File scripts/dev/smoke-resources.ps1 -Name mika-desktop -Seconds 60 [-OutFile amostras.json]
param(
  [Parameter(Mandatory)] [string]$Name,
  [int]$Seconds = 60,
  [string]$OutFile = ''
)

$ErrorActionPreference = 'Stop'
$procName = if ($Name.EndsWith('.exe')) { $Name } else { "$Name.exe" }

$hasGpu = [bool](Get-Command nvidia-smi -ErrorAction SilentlyContinue)
$gpuBefore = if ($hasGpu) { [int](nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits) } else { 0 }

function Snapshot {
  $main = @(Get-CimInstance Win32_Process -Filter "name='$procName'")
  $ids = @($main | ForEach-Object { $_.ProcessId })
  if ($ids.Count -gt 0) {
    $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($ids[0])") |
      Where-Object { $_.Name -eq 'msedgewebview2.exe' }
    $ids += @($children | ForEach-Object { $_.ProcessId })
  }

  $perId = @{}
  $memBytes = 0
  foreach ($procId in $ids) {
    try {
      $p = Get-Process -Id $procId -ErrorAction Stop
      $perId[$procId] = $p.CPU
      $memBytes += $p.WorkingSet64
    } catch { }
  }
  [pscustomobject]@{ at = Get-Date; memBytes = $memBytes; perId = $perId }
}

Write-Host "Amostrando '$Name' por ${Seconds}s (1 amostra/s; filhos msedgewebview2 incluídos)..."
$snapshots = @()
$deadline = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $deadline) {
  $snapshots += Snapshot
  Start-Sleep -Milliseconds 900
}

$useful = @($snapshots | Where-Object { $_.perId.Count -gt 0 })
if ($useful.Count -eq 0) { Write-Error "processo '$Name' não apareceu na janela"; exit 1 }

# CPU: acumulado por PID entre a primeira e a última amostra em que o PID está vivo
# (PID que morre no meio da janela entra como 0 — não incentiva misreport).
$cpuDelta = 0.0
$idsAlive = @{}
foreach ($s in $useful) { foreach ($k in $s.perId.Keys) { $idsAlive[$k] = $true } }
foreach ($procId in $idsAlive.Keys) {
  $first = $null
  $last = $null
  $seen = $false
  foreach ($s in $useful) {
    if ($s.perId.ContainsKey($procId)) {
      $v = $s.perId[$procId]
      if (-not $seen) { $first = $v; $seen = $true }
      $last = $v
    }
  }
  if ($seen) { $cpuDelta += $last - $first }
}
$wallS = ($useful[-1].at - $useful[0].at).TotalSeconds
$cpuPercent = if ($wallS -gt 0) { [math]::Round(100 * $cpuDelta / $wallS, 1) } else { 0 }

$memAvgMB = [math]::Round((($useful | Measure-Object memBytes -Average).Average / 1MB), 1)
$memPeakMB = [math]::Round((($useful | Measure-Object memBytes -Maximum).Maximum / 1MB), 1)

Write-Host ''
Write-Host "== smoke resources '$Name' — janela $([math]::Round($wallS, 0))s, $($useful.Count) amostras =="
Write-Host "CPU: $([math]::Round($cpuDelta, 1)) s de núcleo em $([math]::Round($wallS, 0))s de parede → $cpuPercent % de 1 núcleo"
Write-Host "memória (exe + webview agregado): média $memAvgMB MB · pico $memPeakMB MB"

if ($hasGpu) {
  Start-Sleep -Seconds 2
  $gpuAfter = [int](nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits)
  Write-Host "VRAM do host: antes $gpuBefore MB · em serviço $gpuAfter MB (Δ $([math]::Round($gpuAfter - $gpuBefore, 0)) MB)"
}

if ($OutFile) {
  $useful | ForEach-Object {
    [pscustomobject]@{
      at = $_.at.ToString('o')
      memMB = [math]::Round($_.memBytes / 1MB, 1)
      pids = @($_.perId.Keys | Sort-Object)
    }
  } | ConvertTo-Json -Depth 3 | Set-Content $OutFile
  Write-Host "amostras → $OutFile"
}
