$ErrorActionPreference = "Continue"
$files = [ordered]@{
  "CORE" = "$env:RUNNER_TEMP\phase1-results.json"
  "TRAY-WIN32-MOUSE" = "$env:RUNNER_TEMP\phase1-tray-results.json"
  "TRAY-UIA" = "$env:RUNNER_TEMP\phase1-tray-uia-results.json"
  "TRAY-KEYBOARD" = "$env:RUNNER_TEMP\phase1-tray-keyboard-results.json"
}
$missing=@(); $failed=@(); $report=[ordered]@{}
foreach($group in $files.Keys){
  $file=$files[$group]
  if(!(Test-Path $file)){ $missing += "$group -> $file"; $report[$group]="MISSING"; continue }
  try {
    $data=Get-Content $file -Raw | ConvertFrom-Json
    $g=[ordered]@{PASS=0;FAIL=0;TOTAL=0}
    foreach($p in $data.PSObject.Properties){
      $v=[string]$p.Value
      $g.TOTAL++
      if($v -like "FAIL*"){ $g.FAIL++; $failed += "$group :: $($p.Name): $v" }
      elseif($v -like "PASS*"){ $g.PASS++ }
    }
    $report[$group]=$g
  } catch {
    $missing += "$group -> unreadable result: $($_.Exception.Message)"
    $report[$group]="UNREADABLE"
  }
}
$lines=@("SAEED PHASE 1 TEST REPORT","=========================","Generated: $(Get-Date -Format o)","")
foreach($group in $report.Keys){
  $r=$report[$group]
  if($r -is [System.Collections.IDictionary]){ $lines += ("{0}: TOTAL={1} PASS={2} FAIL={3}" -f $group,$r.TOTAL,$r.PASS,$r.FAIL) }
  else { $lines += "${group}: $r" }
}
$lines += ""; $lines += "FAILURES"; $lines += "--------"
if($failed.Count){$lines += $failed}else{$lines += "None"}
$lines += ""; $lines += "MISSING / UNREADABLE"; $lines += "--------------------"
if($missing.Count){$lines += $missing}else{$lines += "None"}
$report | ConvertTo-Json -Depth 8 | Set-Content "$env:RUNNER_TEMP\phase1-full-report.json" -Encoding utf8
$lines | Set-Content "$env:RUNNER_TEMP\phase1-full-report.txt" -Encoding utf8
Write-Host ($lines -join [Environment]::NewLine)
if($missing.Count -or $failed.Count){ exit 1 } else { exit 0 }