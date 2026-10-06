$ErrorActionPreference = "Stop"
$files = @(
  "$env:RUNNER_TEMP\phase1-results.json",
  "$env:RUNNER_TEMP\phase1-tray-results.json"
)
$missing = @()
$failed = @()
foreach($file in $files){
  if(!(Test-Path $file)){ $missing += $file; continue }
  $data = Get-Content $file -Raw | ConvertFrom-Json
  foreach($p in $data.PSObject.Properties){
    if([string]$p.Value -like "FAIL*"){ $failed += "$($p.Name): $($p.Value)" }
  }
}
if($missing.Count){
  Write-Host "Missing smoke result files:" -ForegroundColor Red
  $missing | ForEach-Object { Write-Host " - $_" -ForegroundColor Red }
  exit 1
}
if($failed.Count){
  Write-Host "Smoke failures:" -ForegroundColor Red
  $failed | ForEach-Object { Write-Host " - $_" -ForegroundColor Red }
  exit 1
}
Write-Host "All core and native Tray smoke checks passed." -ForegroundColor Green
exit 0
