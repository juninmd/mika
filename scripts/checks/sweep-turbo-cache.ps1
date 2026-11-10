<#
Turborepo never prunes .turbo/cache by size or age. This reports what is there
and, with -Apply, drops entries older than -Days plus any orphan .tmp writes.
#>
param([int]$Days = 14, [switch]$Apply)

$cache = Join-Path (Split-Path $PSScriptRoot -Parent | Split-Path -Parent) '.turbo\cache'
if (-not (Test-Path $cache)) { "nada em $cache"; exit 0 }

$cutoff = (Get-Date).AddDays(-$Days)
$all = Get-ChildItem $cache -File -Force -EA SilentlyContinue
$stale = $all | Where-Object { $_.LastWriteTime -lt $cutoff }
$tmp = $all | Where-Object { $_.Name -like '*.tmp' }

"entradas .tar.zst : {0}" -f ($all | Where-Object Extension -eq '.zst').Count
"total             : {0:N2} GB" -f (($all | Measure-Object -Sum Length).Sum / 1GB)
"stale (> $Days d)  : {0:N2} GB em {1} arquivos" -f (($stale | Measure-Object -Sum Length).Sum / 1GB), $stale.Count
".tmp orfaos       : {0:N2} GB em {1} arquivos" -f (($tmp | Measure-Object -Sum Length).Sum / 1GB), $tmp.Count

if ($Apply) {
  $stale + $tmp | Sort-Object FullName -Unique | Remove-Item -Force -Confirm:$false -EA SilentlyContinue
  "restante          : {0:N2} GB" -f ((Get-ChildItem $cache -File -Force -EA SilentlyContinue | Measure-Object -Sum Length).Sum / 1GB)
} else {
  "(rode com -Apply para apagar)"
}
