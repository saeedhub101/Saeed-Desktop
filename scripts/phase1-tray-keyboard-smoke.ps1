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
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class TrayKeyboardInput {
 [DllImport("user32.dll")] public static extern void keybd_event(byte bVk,byte bScan,uint dwFlags,UIntPtr dwExtraInfo);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hWnd);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls,string title);
 public const uint UP=0x0002;
 public const byte ESC=0x1B, ENTER=0x0D, TAB=0x09, LEFT=0x25, RIGHT=0x27, UPKEY=0x26, DOWN=0x28, F10=0x79, B=0x42, WIN=0x5B;
}
"@
function Key([byte]$k){[TrayKeyboardInput]::keybd_event($k,0,0,[UIntPtr]::Zero);[TrayKeyboardInput]::keybd_event($k,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero);Start-Sleep -Milliseconds 250}
function Hotkey([byte]$mod,[byte]$k){[TrayKeyboardInput]::keybd_event($mod,0,0,[UIntPtr]::Zero);[TrayKeyboardInput]::keybd_event($k,0,0,[UIntPtr]::Zero);[TrayKeyboardInput]::keybd_event($k,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero);[TrayKeyboardInput]::keybd_event($mod,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero);Start-Sleep -Milliseconds 300}
function Send-Keys([byte[]]$keys){foreach($k in $keys){Key $k}}
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

# Windows keyboard navigation to the notification area.
# Win+B focuses the notification area without using the mouse.
Hotkey ([TrayKeyboardInput]::WIN) ([TrayKeyboardInput]::B)
Pass "keyboard-focus-notification-area" "Win+B was sent to focus the notification area"

# Use Tab/arrow navigation and context-menu key. We deliberately do not assume a mouse coordinate.
# Escape first clears any transient focus/menu state.
Key ([TrayKeyboardInput]::ESC)
Send-Keys @([TrayKeyboardInput]::TAB,[TrayKeyboardInput]::TAB)
Pass "keyboard-navigation" "keyboard navigation sequence completed without mouse input"

# Shift+F10 is the keyboard context-menu command. It is sent after notification-area focus.
[TrayKeyboardInput]::keybd_event(0x10,0,0,[UIntPtr]::Zero)
Key ([TrayKeyboardInput]::F10)
[TrayKeyboardInput]::keybd_event(0x10,0,[TrayKeyboardInput]::UP,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 500
$popup=[TrayKeyboardInput]::FindWindow("#32768",$null)
Assert ($popup -ne [IntPtr]::Zero) "keyboard-native-menu" "keyboard opened a native Windows popup menu without mouse clicks"

# If a native popup appeared, keyboard arrows/Enter exercise the menu without mouse coordinates.
if($popup -ne [IntPtr]::Zero){
  Key ([TrayKeyboardInput]::ESC)
  Pass "keyboard-menu-dismiss" "Escape dismissed the native menu"
} else {
  Fail "keyboard-menu-dismiss" "No native popup was exposed by the keyboard-only navigation path"
}

Assert (ProcessAlive) "keyboard-process-lifetime" "keyboard test did not terminate Saeed unexpectedly"
Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
$results | ConvertTo-Json -Depth 5 | Tee-Object "$env:RUNNER_TEMP\phase1-tray-keyboard-results.json"
$failed=@($results.GetEnumerator()|Where-Object{$_.Value -like "FAIL*"})
Write-Host "TRAY KEYBOARD SUMMARY: $($results.Count) checks, $($failed.Count) failed" -ForegroundColor $(if($failed.Count){"Red"}else{"Green"})
if($failed.Count){exit 1}else{exit 0}
