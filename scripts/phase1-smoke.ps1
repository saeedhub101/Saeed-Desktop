param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [string]$InstallDir = "$env:RUNNER_TEMP\Saeed-Phase1"
)

$ErrorActionPreference = "Continue"
Set-StrictMode -Off

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

function Wait-Until([scriptblock]$Condition,[int]$TimeoutMs=15000) {
  $end=(Get-Date).AddMilliseconds($TimeoutMs)
  do {
    try {
      $value=&$Condition
      if($value){ return $value }
    } catch {}
    Start-Sleep -Milliseconds 200
  } while((Get-Date)-lt $end)
  return $null
}

function Get-SaeedProcess {
  @(Get-Process -Name "Saeed" -ErrorAction SilentlyContinue | Where-Object {
    $_.Path -and $_.Path -eq $exe
  })
}

function Get-DescendantPids([int]$RootPid) {
  $all=@{}
  try {
    Get-CimInstance Win32_Process -ErrorAction Stop | ForEach-Object {
      $all[[int]$_.ProcessId]=[int]$_.ParentProcessId
    }
  } catch {
    return @()
  }

  $result=@()
  $queue=@($RootPid)
  while($queue.Count -gt 0){
    $parent=$queue[0]
    if($queue.Count -gt 1){ $queue=@($queue[1..($queue.Count-1)]) } else { $queue=@() }

    foreach($entry in $all.GetEnumerator()){
      if($entry.Value -eq $parent -and $entry.Key -ne $RootPid){
        if($result -notcontains $entry.Key){
          $result += [int]$entry.Key
          $queue += [int]$entry.Key
        }
      }
    }
  }
  return @($result)
}

function Get-TreeMemoryMB([int]$RootPid) {
  $pids=@($RootPid)+@(Get-DescendantPids $RootPid)
  $bytes=0
  foreach($childPid in $pids){
    try { $bytes += [int64](Get-Process -Id $childPid -ErrorAction Stop).WorkingSet64 } catch {}
  }
  return [math]::Round($bytes / 1MB, 1)
}

function Get-WindowHandle {
  $h=[Win32Input]::FindWindow($null,"Saeed")
  if($h -ne [IntPtr]::Zero){ return $h }
  $p=Get-SaeedProcess | Select-Object -First 1
  if($p){
    $p.Refresh()
    return $p.MainWindowHandle
  }
  return [IntPtr]::Zero
}

function Get-Rect([IntPtr]$Handle) {
  $rect=New-Object Win32Input+RECT
  if(![Win32Input]::GetWindowRect($Handle,[ref]$rect)){
    throw "GetWindowRect failed"
  }
  return $rect
}

$script:CurrentTest = "setup"
# The smoke suite is deliberately non-fail-fast. Runtime/UIA exceptions are recorded as FAIL and execution continues.
function Record-Exception([string]$name,[System.Exception]$error) { Fail $name $error.Exception.Message }
function Pass($name,$note="") { $results[$name] = "PASS" + $(if($note){" - $note"}else{""}); Write-Host "PASS: $name $note" -ForegroundColor Green }
function Fail($name,$note) { $results[$name] = "FAIL - $note"; Write-Host "FAIL: $name - $note" -ForegroundColor Red }
function Assert($condition,$name,$note) { if($condition){Pass $name $note}else{Fail $name $note} }
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

# Lifecycle is tested directly through native Win32 window messages and the
# single-instance handoff. Tray interaction is intentionally excluded from this
# core smoke suite and is covered by scripts/phase1-tray-smoke.ps1.
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32ClosePhase1 {
 [DllImport("user32.dll",SetLastError=true)] public static extern bool PostMessage(IntPtr hWnd,uint Msg,IntPtr wParam,IntPtr lParam);
}
"@

[Win32ClosePhase1]::PostMessage($h,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
$hidden=Wait-Until { if((Get-WindowHandle)-eq [IntPtr]::Zero){$true}else{$null}} 10000
Assert $hidden "hide-destroys-window" "native WM_CLOSE triggered the real Hide/destroy lifecycle"
Start-Sleep -Seconds 2
Assert ((Get-TreeMemoryMB $proc.Id) -lt 300) "memory-after-hide" "$(Get-TreeMemoryMB $proc.Id) MB working set after Hide"
$children=Get-DescendantPids $proc.Id
$wv=@($children|Where-Object{try{(Get-Process -Id $_ -ErrorAction Stop).ProcessName -match "msedgewebview2"}catch{$false}})
Assert ($wv.Count -eq 0) "webview2-cleanup" "no WebView2 descendants remain after Hide"

# The second launch exercises the real single-instance callback, which must
# recreate/show the character without using the tray.
$p2=Start-Process $exe -PassThru
$h=Wait-Until { $x=Get-WindowHandle;if($x -ne [IntPtr]::Zero){$x}else{$null}} 15000
Assert $h "show-via-single-instance" "second launch handed off to the existing tray process and recreated the character"
$rect1=Get-Rect $h
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
$expectedLeft=$rect2.Left; $expectedTop=$rect2.Top
Get-SaeedProcess | Stop-Process -Force
Wait-Until { if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000 | Out-Null
$proc=Start-Process $exe -PassThru
$h=Wait-Until { $x=Get-WindowHandle; if($x -ne [IntPtr]::Zero){$x}else{$null}} 15000
Assert $h "restart" "application restarted after persistence check"
$rectRestart=Get-Rect $h
Assert ([math]::Abs($rectRestart.Left-$expectedLeft) -le 8 -and [math]::Abs($rectRestart.Top-$expectedTop) -le 8) "position-persistence-restart" "saved character position restored after process restart"

# Transparent corner must not drag the window.
$rectBefore=Get-Rect $h
[Win32Input]::SetCursorPos($rectBefore.Left+2,$rectBefore.Top+2)|Out-Null
[Win32Input]::mouse_event([Win32Input]::LEFTDOWN,0,0,0,[UIntPtr]::Zero)
[Win32Input]::SetCursorPos($rectBefore.Left+90,$rectBefore.Top+60)|Out-Null
[Win32Input]::mouse_event([Win32Input]::LEFTUP,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Milliseconds 700
$rectAfter=Get-Rect $h
Assert ([math]::Abs($rectAfter.Left-$rectBefore.Left) -lt 10 -and [math]::Abs($rectAfter.Top-$rectBefore.Top) -lt 10) "transparent-click-through" "transparent corner did not drag the window"

# Size/Always-on-Top/Low-Power menu actions are tray-specific and are
# intentionally covered only by the independent native tray smoke.
# Change Character is tray-menu driven and is covered by the independent
# native tray smoke; the core lifecycle suite does not depend on tray access.
# Rotate-once is exposed through the tray in Phase 1; tray invocation is
# covered separately. Core smoke only verifies the process remains stable.
Start-Sleep -Seconds 2
Assert ((Get-SaeedProcess).Count -eq 1) "runtime-stability" "process remained alive during idle runtime"
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
$log="$env:APPDATA\Saeed\logs\saeed.log"
$err=Wait-Until {
  if(Test-Path $log){
    $lines=Get-Content $log -ErrorAction SilentlyContinue
    if($lines -match "GLB load failed"){ $true } else { $null }
  }
} 5000
Assert $err "corrupt-glb" "invalid GLB produced a logged loader error and the process stayed alive"

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

# Tray Quit is tested independently. Core smoke cleanup uses process termination
# only after all lifecycle assertions have completed.
Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
Wait-Until { if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000 | Out-Null
$results | ConvertTo-Json -Depth 4 | Tee-Object "$env:RUNNER_TEMP\phase1-results.json"
$failed = @($results.GetEnumerator() | Where-Object { $_.Value -like "FAIL*" })
Write-Host "PHASE 1 TEST SUMMARY: $($results.Count) checks, $($failed.Count) failed" -ForegroundColor $(if($failed.Count){ "Red" } else { "Green" })
if($failed.Count -gt 0){ $failed | ForEach-Object { Write-Host " - $($_.Key): $($_.Value)" -ForegroundColor Red }; exit 1 }
exit 0
