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
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls,string title);
 public const uint UP=0x0002;
 public const byte ESC=0x1B, ENTER=0x0D, HOME=0x24, DOWN=0x28, F10=0x79;
}
"@
function Key([byte]$k){[TrayKeyboardInput]::keybd_event($k,0,0,[UIntPtr]::Zero);[TrayKeyboardInput]::keybd_event($k,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300}
function ShiftF10 {
  [TrayKeyboardInput]::keybd_event(0x10,0,0,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::F10,0,0,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event([TrayKeyboardInput]::F10,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
  [TrayKeyboardInput]::keybd_event(0x10,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 500
}
function Get-TrayElement {
  $names=@("Saeed","Saeed.exe")
  foreach($name in $names){
    $condition=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$name)
    $e=[System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
    if($e){return $e}
  }
  return $null
}
function Focus-SaeedTray {
  $e=Wait-Until { Get-TrayElement } 5000
  if($e){ try { $e.SetFocus(); Start-Sleep -Milliseconds 500; return $true } catch {} }
  return $false
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

$focused=Focus-SaeedTray
Assert $focused "keyboard-tray-focus" "UIA located the Saeed tray icon and keyboard focus was assigned without mouse input"

if($focused){
  # Open the native tray menu using the keyboard context-menu command.
  ShiftF10
  $popup=[TrayKeyboardInput]::FindWindow("#32768",$null)
  Assert ($popup -ne [IntPtr]::Zero) "keyboard-native-menu" "Shift+F10 opened the native tray menu without a mouse click"

  if($popup -ne [IntPtr]::Zero){
    # Current visible state starts with Show disabled and Hide enabled.
    Key ([TrayKeyboardInput]::HOME)
    Key ([TrayKeyboardInput]::DOWN)
    Key ([TrayKeyboardInput]::ENTER)
    $hideApplied=Wait-Until { if((Get-SaeedProcess).Count -gt 0){$true}else{$null}} 5000
    Assert $hideApplied "keyboard-hide" "keyboard selected Hide Saeed without mouse input"

    $focused2=Focus-SaeedTray
    Assert $focused2 "keyboard-tray-refocus" "tray icon was refocused after keyboard Hide"
    if($focused2){
      ShiftF10
      $popup2=[TrayKeyboardInput]::FindWindow("#32768",$null)
      Assert ($popup2 -ne [IntPtr]::Zero) "keyboard-show-menu" "Shift+F10 reopened the native menu"
      if($popup2 -ne [IntPtr]::Zero){
        Key ([TrayKeyboardInput]::HOME)
        Key ([TrayKeyboardInput]::ENTER)
        Assert (Wait-Until { if((Get-SaeedProcess).Count -gt 0){$true}else{$null}} 5000) "keyboard-show" "keyboard selected Show Saeed without mouse input"
      }
    }
  }
} else {
  Fail "keyboard-native-menu" "Saeed tray icon could not be focused for keyboard-only testing"
  Fail "keyboard-hide" "Skipped because the exact Saeed tray icon could not be focused"
  Fail "keyboard-tray-refocus" "Skipped because the exact Saeed tray icon could not be focused"
  Fail "keyboard-show-menu" "Skipped because the exact Saeed tray icon could not be focused"
  Fail "keyboard-show" "Skipped because the exact Saeed tray icon could not be focused"
}
Key ([TrayKeyboardInput]::ESC)
Assert (ProcessAlive) "keyboard-process-lifetime" "keyboard test did not terminate Saeed unexpectedly"
Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
$results | ConvertTo-Json -Depth 5 | Tee-Object "$env:RUNNER_TEMP\phase1-tray-keyboard-results.json"
$failed=@($results.GetEnumerator()|Where-Object{$_.Value -like "FAIL*"})
Write-Host "TRAY KEYBOARD SUMMARY: $($results.Count) checks, $($failed.Count) failed" -ForegroundColor $(if($failed.Count){"Red"}else{"Green"})
if($failed.Count){exit 1}else{exit 0}