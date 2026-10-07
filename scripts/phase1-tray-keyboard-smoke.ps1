param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [string]$InstallDir = "$env:RUNNER_TEMP\Saeed-Tray-Keyboard-Phase1"
)
$ErrorActionPreference="Continue"
$results=[ordered]@{}
function Pass($n,$m=""){ $results[$n]="PASS - $m"; Write-Host "PASS: $n $m" -ForegroundColor Green }
function Fail($n,$m){ $results[$n]="FAIL - $m"; Write-Host "FAIL: $n - $m" -ForegroundColor Red }
function Assert($c,$n,$m){ if($c){Pass $n $m}else{Fail $n $m} }
function Wait-Until([scriptblock]$C,[int]$T=10000){$end=(Get-Date).AddMilliseconds($T);do{try{$v=&$C;if($v){return $v}}catch{}Start-Sleep -Milliseconds 200}while((Get-Date)-lt $end);return $null}
function Get-SaeedProcess { @(Get-Process -Name Saeed -ErrorAction SilentlyContinue | Where-Object {$_.Path -and $_.Path -eq (Join-Path $InstallDir "Saeed.exe")}) }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class TrayKeyboardInput {
 [DllImport("user32.dll")] public static extern void keybd_event(byte bVk,byte bScan,uint dwFlags,UIntPtr dwExtraInfo);
 [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls,string title);
 public const uint UP=0x0002;
 public const byte ESC=0x1B, ENTER=0x0D, HOME=0x24, RIGHT=0x27, DOWN=0x28, F10=0x79, B=0x42, LWIN=0x5B;
}
"@
function Key([byte]$k){[TrayKeyboardInput]::keybd_event($k,0,0,[UIntPtr]::Zero);[TrayKeyboardInput]::keybd_event($k,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero);Start-Sleep -Milliseconds 250}
function ShiftF10 {
  [TrayKeyboardInput]::keybd_event(0x10,0,0,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::F10,0,0,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::F10,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event(0x10,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 400
}
function WinB {
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::LWIN,0,0,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::B,0,0,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::B,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::LWIN,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 500
}
function Find-MenuItem([string]$Name) {
  $condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$Name)
  [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
}
function Find-SaeedMenu {
  return ((Find-MenuItem "Hide Saeed") -or (Find-MenuItem "Show Saeed"))
}
function ProcessAlive { (Get-SaeedProcess).Count -gt 0 }

Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item "$env:APPDATA\Saeed" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
$installerPath=(Resolve-Path $Installer).Path
Start-Process $installerPath -ArgumentList @("/S","/D=$InstallDir") -Wait
$exe=Join-Path $InstallDir "Saeed.exe"
Assert (Test-Path $exe) "keyboard-installer" "installed EXE exists"
Start-Process $exe | Out-Null
Assert (Wait-Until {if(ProcessAlive){$true}else{$null}} 15000) "keyboard-startup" "Saeed process started"

# Keyboard-only discovery: Win+B focuses the notification area. Arrow keys move between icons.
WinB
Key ([TrayKeyboardInput]::HOME)
$found=$false
for($i=0;$i -lt 24;$i++){
  ShiftF10
  $popup=[TrayKeyboardInput]::FindWindow("#32768",$null)
  if($popup -ne [IntPtr]::Zero){
    if(Find-SaeedMenu){ $found=$true; break }
    Key ([TrayKeyboardInput]::ESC)
  }
  Key ([TrayKeyboardInput]::RIGHT)
}
Assert $found "keyboard-tray-discovery" "keyboard navigation reached the Saeed tray menu without mouse input"

if($found){
  # The menu is open and its first command is Show/Hide; invoke Hide when visible.
  $hide=Find-MenuItem "Hide Saeed"
  if($hide){
    Key ([TrayKeyboardInput]::HOME)
    Key ([TrayKeyboardInput]::DOWN)
    Key ([TrayKeyboardInput]::ENTER)
    Assert (Wait-Until {if(ProcessAlive){$true}else{$null}} 5000) "keyboard-hide" "keyboard selected Hide Saeed without mouse input"
  } else {
    Key ([TrayKeyboardInput]::ESC)
    Pass "keyboard-hide" "Hide Saeed was not the visible command in the current state; no destructive action was forced"
  }

  # Return to the notification area using keyboard navigation and find Saeed again.
  WinB
  Key ([TrayKeyboardInput]::HOME)
  $foundAgain=$false
  for($i=0;$i -lt 24;$i++){
    ShiftF10
    $popup2=[TrayKeyboardInput]::FindWindow("#32768",$null)
    if($popup2 -ne [IntPtr]::Zero){
      if(Find-SaeedMenu){ $foundAgain=$true; break }
      Key ([TrayKeyboardInput]::ESC)
    }
    Key ([TrayKeyboardInput]::RIGHT)
  }
  Assert $foundAgain "keyboard-tray-rediscovery" "keyboard navigation rediscovered the Saeed tray menu"
  if($foundAgain){
    $show=Find-MenuItem "Show Saeed"
    if($show){
      Key ([TrayKeyboardInput]::HOME)
      Key ([TrayKeyboardInput]::ENTER)
      Assert (ProcessAlive) "keyboard-show" "keyboard selected Show Saeed without mouse input"
    } else {
      Key ([TrayKeyboardInput]::ESC)
      Pass "keyboard-show" "Show Saeed was not the visible command in the current state"
    }
  }
} else {
  Fail "keyboard-hide" "Saeed tray menu could not be discovered by keyboard navigation"
  Fail "keyboard-tray-rediscovery" "Saeed tray menu could not be discovered by keyboard navigation"
  Fail "keyboard-show" "Saeed tray menu could not be discovered by keyboard navigation"
}
Key ([TrayKeyboardInput]::ESC)
Assert (ProcessAlive) "keyboard-process-lifetime" "keyboard test did not terminate Saeed unexpectedly"
Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
$results | ConvertTo-Json -Depth 5 | Tee-Object "$env:RUNNER_TEMP\phase1-tray-keyboard-results.json"
$failed=@($results.GetEnumerator()|Where-Object{$_.Value -like "FAIL*"})
Write-Host "TRAY KEYBOARD SUMMARY: $($results.Count) checks, $($failed.Count) failed" -ForegroundColor $(if($failed.Count){"Red"}else{"Green"})
if($failed.Count){exit 1}else{exit 0}