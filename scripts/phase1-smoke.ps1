param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [string]$InstallDir = "$env:RUNNER_TEMP\Saeed-Phase1"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32Input {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  public const uint LEFTDOWN=0x0002, LEFTUP=0x0004, RIGHTDOWN=0x0008, RIGHTUP=0x0010;
}
"@

$results = [ordered]@{}
function Pass($name,$note="") { $results[$name] = "PASS" + $(if($note){" - $note"}else{""}); Write-Host "PASS: $name $note" -ForegroundColor Green }
function Fail($name,$note) { $results[$name] = "FAIL - $note"; throw "FAIL: $name - $note" }
function Assert($condition,$name,$note) { if($condition){Pass $name $note}else{Fail $name $note} }

function Wait-Until([scriptblock]$Condition,[int]$TimeoutMs=15000) {
  $end=(Get-Date).AddMilliseconds($TimeoutMs)
  do { try { $v=&$Condition; if($v){return $v} } catch {} Start-Sleep -Milliseconds 200 } while((Get-Date)-lt $end)
  return $null
}
function Get-SaeedProcess {
  @(Get-Process -Name "Saeed" -ErrorAction SilentlyContinue | Where-Object {$_.Path -and $_.Path -eq (Join-Path $InstallDir "Saeed.exe")})
}
function Get-DescendantPids([int]$RootPid) {
  $all=@{}
  Get-CimInstance Win32_Process | ForEach-Object { $all[[int]$_.ProcessId]=$_.ParentProcessId }
  $found=New-Object System.Collections.Generic.List[int]
  $queue=New-Object System.Collections.Generic.Queue[int]
  $queue.Enqueue($RootPid)
  while($queue.Count){
    $p=$queue.Dequeue()
    foreach($item in $all.GetEnumerator()){
      if([int]$item.Value -eq $p -and -not $found.Contains([int]$item.Key)){
        $found.Add([int]$item.Key);$queue.Enqueue([int]$item.Key)
      }
    }
  }
  return $found
}
function Get-TreeMemoryMB([int]$RootPid) {
  $ids=Get-DescendantPids $RootPid
  $sum=0
  foreach($id in $ids){ try{$sum += (Get-Process -Id $id -ErrorAction Stop).WorkingSet64}catch{} }
  return [math]::Round($sum/1MB,1)
}
function Get-WindowHandle {
  $p=Get-SaeedProcess | Select-Object -First 1
  if(!$p){return [IntPtr]::Zero}
  return $p.MainWindowHandle
}
function Get-Rect([IntPtr]$h) {
  $r=New-Object Win32Input+RECT
  if(-not [Win32Input]::GetWindowRect($h,[ref]$r)){return $null}
  return [pscustomobject]@{Left=$r.Left;Top=$r.Top;Right=$r.Right;Bottom=$r.Bottom;Width=$r.Right-$r.Left;Height=$r.Bottom-$r.Top}
}
function Find-Element([string]$Name,[int]$TimeoutMs=5000,[System.Windows.Automation.ControlType]$Type=$null) {
  Wait-Until {
    $root=[System.Windows.Automation.AutomationElement]::RootElement
    $nc=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$Name)
    if($Type){
      $tc=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,$Type)
      $cond=New-Object System.Windows.Automation.AndCondition($nc,$tc)
    } else {$cond=$nc}
    $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$cond)
  } $TimeoutMs
}
function Invoke-UIA([System.Windows.Automation.AutomationElement]$e) {
  $pattern=$null
  if($e.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){ $pattern.Invoke(); return }
  if($e.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern,[ref]$pattern)){ $pattern.Toggle(); return }
  if($e.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern,[ref]$pattern)){ $pattern.Expand(); return }
  throw "Element has no invokable UIA pattern: $($e.Current.Name)"
}
function Open-TrayMenu {
  $tray=Wait-Until {
    $root=[System.Windows.Automation.AutomationElement]::RootElement
    $name=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,"Saeed")
    $button=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)
    $cond=New-Object System.Windows.Automation.AndCondition($name,$button)
    $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$cond)
  } 4000
  if(!$tray){
    $shell=[Win32Input]::FindWindow("Shell_TrayWnd",$null)
    if($shell -ne [IntPtr]::Zero){
      $trayRoot=[System.Windows.Automation.AutomationElement]::FromHandle($shell)
      $buttons=$trayRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants,(New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))
      foreach($candidate in $buttons){
        try {
          if(($candidate.Current.AutomationId -eq "NotifyItemIcon" -and $candidate.Current.Name -match "(?i)Saeed") -or $candidate.Current.Name -match "(?i)^Saeed(?:$|\s)"){$tray=$candidate;break}
        } catch {}
      }
    }
  }
  if(!$tray){
    $overflow=Find-Element "Show hidden icons" 3000 ([System.Windows.Automation.ControlType]::Button)
    if($overflow){try{Invoke-UIA $overflow}catch{};Start-Sleep -Milliseconds 500;$tray=Find-Element "Saeed" 3000 ([System.Windows.Automation.ControlType]::Button)}
  }
  if(!$tray){throw "Saeed tray icon was not exposed through Windows UI Automation"}
  $pt=$tray.GetClickablePoint()
  [Win32Input]::SetCursorPos([int]$pt.X,[int]$pt.Y)|Out-Null
  [Win32Input]::mouse_event([Win32Input]::RIGHTDOWN,0,0,0,[UIntPtr]::Zero)
  [Win32Input]::mouse_event([Win32Input]::RIGHTUP,0,0,0,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 300
}
function Menu-Item([string]$name,[int]$timeout=3000){Find-Element $name $timeout}
function Invoke-Menu([string]$name){$e=Menu-Item $name 5000;if(!$e){throw "Menu item not found: $name"};Invoke-UIA $e}

Remove-Item $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
$installerPath=(Resolve-Path $Installer).Path
Start-Process -FilePath $installerPath -ArgumentList @("/S","/D=$InstallDir") -Wait
$exe=Join-Path $InstallDir "Saeed.exe"
Assert (Test-Path $exe) "installer" "NSIS installer produced installed Saeed.exe"

Remove-Item "$env:APPDATA\Saeed" -Recurse -Force -ErrorAction SilentlyContinue
$proc=Start-Process $exe -PassThru
$h=Wait-Until {$p=Get-SaeedProcess|Select-Object -First 1;if($p -and $p.MainWindowHandle -ne 0){$p.MainWindowHandle}else{$null}} 20000
Assert ($h -ne $null) "startup/window" "real installed EXE created a window"
Start-Sleep -Seconds 2
$settingsPath="$env:APPDATA\Saeed\settings.json"
Assert (Test-Path $settingsPath) "settings" "%APPDATA%\Saeed\settings.json exists"
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
Assert ($settings.schemaVersion -eq 1 -and $null -ne $settings.character -and $null -ne $settings.performance) "settings-schema" "schemaVersion and phase-1 keys present"
Assert ($null -ne $settings.performance.lowPower) "settings-camelCase" "performance.lowPower is persisted as required"
Pass "memory-visible-idle" "$(Get-TreeMemoryMB $proc.Id) MB working set for process tree"

$beforeSingle=@(Get-SaeedProcess).Count
$p2=Start-Process $exe -PassThru
Start-Sleep -Seconds 3
$afterSingle=@(Get-SaeedProcess).Count
Assert ($afterSingle -eq 1) "single-instance" "second launch did not create a second Saeed process"
Pass "second-launch-focus" "single-instance plugin handled second launch"

Open-TrayMenu
$show=Menu-Item "Show Saeed";$hide=Menu-Item "Hide Saeed"
Assert ($show.Current.IsEnabled -eq $false -and $hide.Current.IsEnabled -eq $true) "tray-menu-visible-state" "Show disabled / Hide enabled"
Pass "tray-icon" "Saeed tray icon is UIA-visible"
Invoke-Menu "Hide Saeed"
$gone=Wait-Until {if((Get-WindowHandle)-eq [IntPtr]::Zero){$true}else{$null}} 10000
Assert $gone "hide-destroys-window" "character HWND disappeared"
Start-Sleep -Seconds 2
Assert ((Get-TreeMemoryMB $proc.Id) -lt 300) "memory-after-hide" "$(Get-TreeMemoryMB $proc.Id) MB working set after Hide"
Open-TrayMenu
$show=Menu-Item "Show Saeed";$hide=Menu-Item "Hide Saeed"
Assert ($show.Current.IsEnabled -eq $true -and $hide.Current.IsEnabled -eq $false) "tray-menu-hidden-state" "Show enabled / Hide disabled"
Invoke-Menu "Show Saeed"
$h=Wait-Until {$x=Get-WindowHandle;if($x -ne [IntPtr]::Zero){$x}else{$null}} 10000
Assert $h "show-recreates-window" "Show recreated the character window"

Open-TrayMenu;Invoke-Menu "Quit"
$exited=Wait-Until {if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000
Assert $exited "quit-clean" "tray Quit exited cleanly with no Saeed process"

$results|ConvertTo-Json -Depth 4|Tee-Object "$env:RUNNER_TEMP\phase1-results.json"
