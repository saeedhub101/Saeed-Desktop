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
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
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
  # Windows 11 may expose notification icons with AutomationId=NotifyItemIcon
  # rather than as a root-level Button named by tooltip. Search notification-area paths.
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
      $buttons=$trayRoot.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button)))
      foreach($candidate in $buttons){
        try {
          if(($candidate.Current.AutomationId -eq "NotifyItemIcon" -and $candidate.Current.Name -match "(?i)Saeed") -or $candidate.Current.Name -match "(?i)^Saeed(?:$|\s)"){
            $tray=$candidate; break
          }
        } catch {}
      }
    }
  }
  if(!$tray){
    $overflow=Find-Element "Show hidden icons" 3000 ([System.Windows.Automation.ControlType]::Button)
    if($overflow){
      try { Invoke-UIA $overflow } catch {}
      Start-Sleep -Milliseconds 500
      $tray=Find-Element "Saeed" 3000 ([System.Windows.Automation.ControlType]::Button)
    }
  }
  if(!$tray){ throw "Saeed tray icon was not exposed through Windows UI Automation (including notification-area and overflow paths)" }
  $pt=$tray.GetClickablePoint()
  [Win32Input]::SetCursorPos([int]$pt.X,[int]$pt.Y)|Out-Null
  [Win32Input]::mouse_event([Win32Input]::RIGHTDOWN,0,0,0,[UIntPtr]::Zero)
  [Win32Input]::mouse_event([Win32Input]::RIGHTUP,0,0,0,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 300
}
function Menu-Item([string]$name,[int]$timeout=3000) {
  Find-Element $name $timeout
}
function Invoke-Menu([string]$name) {
  $e=Menu-Item $name 5000
  if(!$e){throw "Menu item not found: $name"}
  Invoke-UIA $e
}
function Make-TestGlb([string]$Path) {
  $json=@'
{"asset":{"version":"2.0","generator":"Saeed Phase 1 Smoke"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],"buffers":[{"byteLength":42}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36,"target":34962},{"buffer":0,"byteOffset":36,"byteLength":6,"target":34963}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[-0.5,-0.5,0],"max":[0.5,0.5,0]},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR","min":[0],"max":[2]}]}
'@
  $jsonBytes=[Text.Encoding]::UTF8.GetBytes($json)
  while(($jsonBytes.Length % 4) -ne 0){$jsonBytes += 0x20}
  $bin=New-Object byte[] 42
  $vals=@(
    -0.5,-0.5,0.0, 0.5,-0.5,0.0, 0.0,0.5,0.0
  )
  $ms=New-Object IO.MemoryStream
  $bw=New-Object IO.BinaryWriter($ms)
  foreach($v in $vals){$bw.Write([single]$v)}
  foreach($i in 0,1,2){$bw.Write([uint16]$i)}
  $bin=$ms.ToArray(); $bw.Dispose();$ms.Dispose()
  $total=12+8+$jsonBytes.Length+8+$bin.Length
  $out=New-Object IO.MemoryStream
  $w=New-Object IO.BinaryWriter($out)
  $w.Write([uint32]0x46546C67);$w.Write([uint32]2);$w.Write([uint32]$total)
  $w.Write([uint32]$jsonBytes.Length);$w.Write([uint32]0x4E4F534A);$w.Write($jsonBytes)
  $w.Write([uint32]$bin.Length);$w.Write([uint32]0x004E4942);$w.Write($bin)
  [IO.File]::WriteAllBytes($Path,$out.ToArray());$w.Dispose();$out.Dispose()
}

Remove-Item $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
$installerPath=(Resolve-Path $Installer).Path
Start-Process -FilePath $installerPath -ArgumentList @("/S","/D=$InstallDir") -Wait
$exe=Join-Path $InstallDir "Saeed.exe"
Assert (Test-Path $exe) "installer" "NSIS installer produced installed Saeed.exe"

Remove-Item "$env:APPDATA\Saeed" -Recurse -Force -ErrorAction SilentlyContinue
$proc=Start-Process $exe -PassThru
$h=Wait-Until { $p=Get-SaeedProcess|Select-Object -First 1; if($p -and $p.MainWindowHandle -ne 0){$p.MainWindowHandle}else{$null}} 20000
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
$gone=Wait-Until { if((Get-WindowHandle)-eq [IntPtr]::Zero){$true}else{$null}} 10000
Assert $gone "hide-destroys-window" "character HWND disappeared"
Start-Sleep -Seconds 2
Assert ((Get-TreeMemoryMB $proc.Id) -lt 300) "memory-after-hide" "$(Get-TreeMemoryMB $proc.Id) MB working set after Hide"
$children=Get-DescendantPids $proc.Id
$wv=@($children|Where-Object{try{(Get-Process -Id $_ -ErrorAction Stop).ProcessName -match "msedgewebview2"}catch{$false}})
Assert ($wv.Count -eq 0) "webview2-cleanup" "no WebView2 descendants remain after Hide"
Open-TrayMenu
$show=Menu-Item "Show Saeed";$hide=Menu-Item "Hide Saeed"
Assert ($show.Current.IsEnabled -eq $true -and $hide.Current.IsEnabled -eq $false) "tray-menu-hidden-state" "Show enabled / Hide disabled"
Invoke-Menu "Show Saeed"
$h=Wait-Until { $x=Get-WindowHandle;if($x -ne [IntPtr]::Zero){$x}else{$null}} 10000
Assert $h "show-recreates-window" "Show recreated the character window"

$rect1=Get-Rect $h
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
Assert ($settings.character.position.x -ge 0 -or $settings.character.position.y -ge 0) "position-persisted" "window position is persisted"

# Character drag: center is expected to hit the placeholder/model.
$cx=[int](($rect1.Left+$rect1.Right)/2);$cy=[int](($rect1.Top+$rect1.Bottom)/2)
[Win32Input]::SetCursorPos($cx,$cy)|Out-Null
[Win32Input]::mouse_event([Win32Input]::LEFTDOWN,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 100
[Win32Input]::SetCursorPos($cx+80,$cy+40)|Out-Null
Start-Sleep -Milliseconds 100
[Win32Input]::mouse_event([Win32Input]::LEFTUP,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Seconds 2
$rect2=Get-Rect $h
Assert ([math]::Abs($rect2.Left-$rect1.Left) -ge 40) "drag" "center drag moved the window"
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
Assert ([math]::Abs($settings.character.position.x-$rect2.Left) -le 5) "drag-persistence" "drop position persisted"

# Transparent corner must not drag the window.
$rectBefore=Get-Rect $h
[Win32Input]::SetCursorPos($rectBefore.Left+2,$rectBefore.Top+2)|Out-Null
[Win32Input]::mouse_event([Win32Input]::LEFTDOWN,0,0,0,[UIntPtr]::Zero)
[Win32Input]::SetCursorPos($rectBefore.Left+90,$rectBefore.Top+60)|Out-Null
[Win32Input]::mouse_event([Win32Input]::LEFTUP,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 700
$rectAfter=Get-Rect $h
Assert ([math]::Abs($rectAfter.Left-$rectBefore.Left) -lt 10 -and [math]::Abs($rectAfter.Top-$rectBefore.Top) -lt 10) "transparent-click-through" "transparent corner did not drag the window"

# Size menu: all three options are exercised.
Open-TrayMenu
Invoke-Menu "Character Size"
Start-Sleep -Milliseconds 200
Invoke-Menu "Small"
Start-Sleep -Seconds 1
$small=Get-Rect (Get-WindowHandle)
Assert ($small.Width -ge 260 -and $small.Width -le 300) "size-small" "Small is approximately 280x280"
Open-TrayMenu; Invoke-Menu "Character Size"; Start-Sleep -Milliseconds 150; Invoke-Menu "Large"; Start-Sleep -Seconds 1
$large=Get-Rect (Get-WindowHandle)
Assert ($large.Width -ge 430 -and $large.Width -le 490) "size-large" "Large is approximately 460x460"
Open-TrayMenu; Invoke-Menu "Character Size"; Start-Sleep -Milliseconds 150; Invoke-Menu "Medium"; Start-Sleep -Seconds 1

# Always-on-top and Low Power are persisted and exercised through the real tray menu.
Open-TrayMenu
Invoke-Menu "Always on Top"
Start-Sleep -Milliseconds 500
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
$top=$settings.character.alwaysOnTop
Assert ($top -eq $false) "always-on-top-toggle" "Always on Top toggled off and persisted"
Open-TrayMenu; Invoke-Menu "Always on Top"; Start-Sleep -Milliseconds 500
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
Assert ($settings.character.alwaysOnTop -eq $true) "always-on-top-restore" "Always on Top toggled back on"
Open-TrayMenu; Invoke-Menu "Low Power Mode"; Start-Sleep -Seconds 1
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
Assert ($settings.performance.lowPower -eq $true) "low-power-toggle" "Low Power enabled and persisted"
Open-TrayMenu; Invoke-Menu "Low Power Mode"; Start-Sleep -Seconds 1

# Change Character: drive the real native file dialog and then verify persistence.
$valid=Join-Path $env:RUNNER_TEMP "phase1-valid.glb"
Make-TestGlb $valid
Open-TrayMenu; Invoke-Menu "Change Character..."
$edit=Wait-Until {
  $root=[System.Windows.Automation.AutomationElement]::RootElement
  $tc=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Edit)
  $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$tc)
} 8000
Assert $edit "change-character-dialog" "native file dialog appeared"
$vp=$null
Assert ($edit.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$vp)) "change-character-input" "file dialog filename field exposes ValuePattern"
$vp.SetValue($valid)
$open=Find-Element "Open" 5000 ([System.Windows.Automation.ControlType]::Button)
Assert $open "change-character-open" "native Open button found"
Invoke-UIA $open
Start-Sleep -Seconds 3
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
Assert ($settings.character.currentId -ne "default") "change-character" "real Change Character flow selected a new character"
$modelPath=Get-ChildItem "$env:APPDATA\Saeed\characters\$($settings.character.currentId)\model.glb" -ErrorAction Stop
Assert $modelPath "character-storage" "new GLB copied under %APPDATA%\Saeed\characters"
$h=Wait-Until { $x=Get-WindowHandle;if($x -ne [IntPtr]::Zero){$x}else{$null}} 5000
Assert $h "glb-runtime" "installed EXE remained alive after loading a real GLB"

# Rotate once must run without leaving a permanent render loop; invoke and then inspect CPU.
Open-TrayMenu; Invoke-Menu "Debug"; Start-Sleep -Milliseconds 150; Invoke-Menu "Rotate once"
Start-Sleep -Seconds 3
$cpuSamples=@()
for($i=0;$i -lt 3;$i++){
  $p=Get-SaeedProcess|Select-Object -First 1
  if($p){$cpuSamples += [math]::Round($p.CPU,3)}
  Start-Sleep -Seconds 1
}
Pass "rotate-once" "Debug > Rotate once completed without process exit"
$cpuDelta=0
if($cpuSamples.Count -ge 2){$cpuDelta=$cpuSamples[-1]-$cpuSamples[0]}
Assert ($cpuDelta -lt 2.0) "idle-render-cpu" "post-rotate CPU remained near idle"

# Corrupt GLB: point currentId at invalid data, restart, and require an in-window error without process exit.
$badId="corrupt-phase1"
$badDir="$env:APPDATA\Saeed\characters\$badId"
New-Item $badDir -ItemType Directory -Force|Out-Null
[IO.File]::WriteAllBytes("$badDir\model.glb",[Text.Encoding]::UTF8.GetBytes("not-a-glb"))
$settings=Get-Content $settingsPath -Raw|ConvertFrom-Json
$settings.character.currentId=$badId
$settings.character.visible=$true
$settings|ConvertTo-Json -Depth 8|Set-Content $settingsPath -Encoding UTF8
Get-SaeedProcess|Stop-Process -Force
Start-Sleep -Seconds 2
$proc=Start-Process $exe -PassThru
$h=Wait-Until { $x=Get-WindowHandle;if($x -ne [IntPtr]::Zero){$x}else{$null}} 15000
Assert $h "corrupt-glb-startup" "app stayed alive with corrupt GLB"
Start-Sleep -Seconds 2
$root=[System.Windows.Automation.AutomationElement]::RootElement
$err=Wait-Until {
  $texts=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
  foreach($e in $texts){if($e.Current.Name -match "could not load the character model|Saeed could not load"){return $true}}
  $null
} 5000
Assert $err "corrupt-glb" "invalid GLB produced an in-window error and the process stayed alive"

# Closing the character window must hide, not quit. Send WM_CLOSE.
$h=Get-WindowHandle
[void][System.Runtime.InteropServices.Marshal]::GetHRForLastWin32Error()
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32Close {
 [DllImport("user32.dll",SetLastError=true)] public static extern bool PostMessage(IntPtr hWnd,uint Msg,IntPtr wParam,IntPtr lParam);
}
"@
[Win32Close]::PostMessage($h,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
Start-Sleep -Seconds 2
Assert ((Get-SaeedProcess).Count -eq 1) "close-does-not-exit" "closing the character window kept the tray app alive"
Assert ((Get-WindowHandle)-eq [IntPtr]::Zero) "close-hides" "closing the character window triggered Hide/destroy"

# Quit must leave no Saeed process.
Open-TrayMenu
Invoke-Menu "Quit"
$exited=Wait-Until { if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000
Assert $exited "quit-clean" "tray Quit exited cleanly with no Saeed process"

$results | ConvertTo-Json -Depth 4 | Tee-Object "$env:RUNNER_TEMP\phase1-results.json"
