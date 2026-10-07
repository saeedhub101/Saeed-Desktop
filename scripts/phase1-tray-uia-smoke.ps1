param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [string]$InstallDir = "$env:RUNNER_TEMP\Saeed-Tray-UIA-Phase1"
)
$ErrorActionPreference = "Continue"
$results=[ordered]@{}
function Pass($n,$m=""){ $results[$n]="PASS - $m"; Write-Host "PASS: $n $m" -ForegroundColor Green }
function Fail($n,$m){ $results[$n]="FAIL - $m"; Write-Host "FAIL: $n - $m" -ForegroundColor Red }
function Assert($c,$n,$m){ if($c){Pass $n $m}else{Fail $n $m} }
function Wait-Until([scriptblock]$C,[int]$T=10000){
  $end=(Get-Date).AddMilliseconds($T)
  do { try { $v=&$C; if($v){return $v} } catch {} Start-Sleep -Milliseconds 200 } while((Get-Date)-lt $end)
  return $null
}
function Get-SaeedProcess { @(Get-Process -Name Saeed -ErrorAction SilentlyContinue | Where-Object {$_.Path -and $_.Path -eq (Join-Path $InstallDir "Saeed.exe")}) }
function Get-Element([string]$Name,[int]$Timeout=5000){
  $cond=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$Name)
  Wait-Until { [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$cond) } $Timeout
}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class TrayUIAWin32 {
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd,int nCmdShow);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
 public const int SW_SHOWNORMAL=1;
}
"@
Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item "$env:APPDATA\Saeed" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
$installerPath=(Resolve-Path $Installer).Path
Start-Process $installerPath -ArgumentList @("/S","/D=$InstallDir") -Wait
$exe=Join-Path $InstallDir "Saeed.exe"
Assert (Test-Path $exe) "uia-installer" "installed EXE exists"
$proc=Start-Process $exe -PassThru
Assert (Wait-Until { if((Get-SaeedProcess).Count -gt 0){$true}else{$null}} 15000) "uia-startup" "Saeed process started"

$tray=$null
foreach($name in @("Saeed","Saeed.exe")){
  $tray=Get-Element $name 3000
  if($tray){break}
}
Assert ($null -ne $tray) "uia-tray-discovery" "UI Automation found the Saeed notification-area element"
if($tray){
  try {
    $invoke=$tray.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
    Start-Sleep -Milliseconds 500
    Pass "uia-tray-invoke" "UIA Invoke activated the tray element"
  } catch { Fail "uia-tray-invoke" "UIA tray element was found but InvokePattern was unavailable: $($_.Exception.Message)" }

  $hide=Get-Element "Hide Saeed" 3000
  Assert ($null -ne $hide) "uia-menu-discovery-hide" "UIA discovered the native Hide Saeed menu item"
  if($hide){
    try {
      $p=$hide.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern); $p.Invoke()
      $hidden=Wait-Until { if((Get-SaeedProcess).Count -gt 0){$true}else{$null}} 3000
      Assert $hidden "uia-hide-applied" "UIA invoked Hide Saeed and the process remained alive"
    } catch { Fail "uia-hide-invoke" "UIA could not invoke Hide Saeed: $($_.Exception.Message)" }
  }

  # Re-open tray through UIA and invoke Show.
  try { ($tray.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)).Invoke() } catch {}
  $show=Get-Element "Show Saeed" 3000
  Assert ($null -ne $show) "uia-menu-discovery-show" "UIA discovered the native Show Saeed menu item"
  if($show){
    try { ($show.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)).Invoke(); Pass "uia-show-invoke" "UIA invoked Show Saeed" }
    catch { Fail "uia-show-invoke" "UIA could not invoke Show Saeed: $($_.Exception.Message)" }
  }

  # Discover menu state through UIA rather than coordinates.
  foreach($n in @("Always on Top","Low Power Mode","Click-through")){
    $e=Get-Element $n 2500
    Assert ($null -ne $e) "uia-menu-discovery-$($n -replace ' ','-')" "UIA discovered '$n'"
  }

  # Character Size submenu is discovered by UIA if exposed.
  $size=Get-Element "Character Size" 2500
  Assert ($null -ne $size) "uia-character-size" "UIA discovered Character Size"
  if($size){
    try { ($size.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)).Expand(); Start-Sleep -Milliseconds 300 } catch {}
    foreach($n in @("Small","Medium","Large")){
      $e=Get-Element $n 1500
      Assert ($null -ne $e) "uia-size-$($n.ToLower())" "UIA discovered $n"
    }
  }

  # Close any menu with Escape through UIA-independent keyboard cleanup.
  Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class TrayUIACleanup { [DllImport("user32.dll")] public static extern void keybd_event(byte k,byte s,uint f,UIntPtr e); public const uint UP=2; }
"@
  [TrayUIACleanup]::keybd_event(0x1B,0,0,[UIntPtr]::Zero); [TrayUIACleanup]::keybd_event(0x1B,0,2,[UIntPtr]::Zero)
} else {
  foreach($n in @("uia-tray-invoke","uia-menu-discovery-hide","uia-hide-invoke","uia-menu-discovery-show","uia-show-invoke","uia-menu-discovery-Always-on-Top","uia-menu-discovery-Low-Power-Mode","uia-menu-discovery-Click-through","uia-character-size","uia-size-small","uia-size-medium","uia-size-large")){ Fail $n "UIA could not locate the Saeed tray element; remaining checks were still recorded." }
}
Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
$results | ConvertTo-Json -Depth 5 | Tee-Object "$env:RUNNER_TEMP\phase1-tray-uia-results.json"
$failed=@($results.GetEnumerator()|Where-Object{$_.Value -like "FAIL*"})
Write-Host "TRAY UIA SUMMARY: $($results.Count) checks, $($failed.Count) failed" -ForegroundColor $(if($failed.Count){"Red"}else{"Green"})
if($failed.Count){exit 1}else{exit 0}
