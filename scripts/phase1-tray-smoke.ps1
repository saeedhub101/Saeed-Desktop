param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [string]$InstallDir = "$env:RUNNER_TEMP\Saeed-Tray-Phase1"
)

$ErrorActionPreference = "Continue"
$results = [ordered]@{}
function Pass($name,$note=""){ $results[$name]="PASS - $note"; Write-Host "PASS: $name $note" -ForegroundColor Green }
function Fail($name,$note){ $results[$name]="FAIL - $note"; Write-Host "FAIL: $name - $note" -ForegroundColor Red }
function Assert($condition,$name,$note){ if($condition){Pass $name $note}else{Fail $name $note} }
function Wait-Until([scriptblock]$Condition,[int]$TimeoutMs=15000){
  $end=(Get-Date).AddMilliseconds($TimeoutMs)
  do { try { $v=&$Condition; if($v){return $v} } catch {} Start-Sleep -Milliseconds 200 } while((Get-Date)-lt $end)
  return $null
}
function Get-SaeedProcess { @(Get-Process -Name "Saeed" -ErrorAction SilentlyContinue | Where-Object {$_.Path -and $_.Path -eq (Join-Path $InstallDir "Saeed.exe")}) }
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class TraySmokeWin32 {
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string lpClassName,string lpWindowName);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hWnd);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd,out RECT rect);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int X,int Y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd,System.Text.StringBuilder lpClassName,int nMaxCount);
  [DllImport("user32.dll")] public static extern int GetMenuItemCount(IntPtr hMenu);
 [DllImport("user32.dll")] public static extern bool GetMenuItemRect(IntPtr hWnd,IntPtr hMenu,uint nPos,out RECT rect);
 [DllImport("user32.dll")] public static extern IntPtr GetMenu(IntPtr hWnd);

 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
 public const uint LEFTDOWN=0x0002,LEFTUP=0x0004,RIGHTDOWN=0x0008,RIGHTUP=0x0010;
}
"@

function Get-WindowHandle {
  $h=[TraySmokeWin32]::FindWindow($null,"Saeed")
  if($h -ne [IntPtr]::Zero -and [TraySmokeWin32]::IsWindow($h)){return $h}
  $p=Get-SaeedProcess|Select-Object -First 1
  if(!$p){return [IntPtr]::Zero}
  $p.Refresh(); $candidate=$p.MainWindowHandle
  if($candidate -ne [IntPtr]::Zero -and [TraySmokeWin32]::IsWindow($candidate)){return $candidate}
  return [IntPtr]::Zero
}
function ClickPoint([int]$x,[int]$y,[bool]$right=$false){
  [TraySmokeWin32]::SetCursorPos($x,$y)|Out-Null
  if($right){
    [TraySmokeWin32]::mouse_event([TraySmokeWin32]::RIGHTDOWN,0,0,0,[UIntPtr]::Zero)
    [TraySmokeWin32]::mouse_event([TraySmokeWin32]::RIGHTUP,0,0,0,[UIntPtr]::Zero)
  } else {
    [TraySmokeWin32]::mouse_event([TraySmokeWin32]::LEFTDOWN,0,0,0,[UIntPtr]::Zero)
    [TraySmokeWin32]::mouse_event([TraySmokeWin32]::LEFTUP,0,0,0,[UIntPtr]::Zero)
  }
}
function Get-MenuPopup {
  $h=[TraySmokeWin32]::FindWindow("#32768",$null)
  if($h -ne [IntPtr]::Zero){return $h}
  $fg=[TraySmokeWin32]::GetForegroundWindow()
  if($fg -ne [IntPtr]::Zero){
    $name=New-Object System.Text.StringBuilder 64
    [TraySmokeWin32]::GetClassName($fg,$name,$name.Capacity)|Out-Null
    if($name.ToString() -eq "#32768"){return $fg}
  }
  return [IntPtr]::Zero
}

Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
Wait-Until { if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000 | Out-Null
Remove-Item "$env:APPDATA\Saeed" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
$installerPath=(Resolve-Path $Installer).Path
Start-Process -FilePath $installerPath -ArgumentList @("/S","/D=$InstallDir") -Wait
$exe=Join-Path $InstallDir "Saeed.exe"
Assert (Test-Path $exe) "installer" "installed EXE exists"

Remove-Item "$env:APPDATA\Saeed" -Recurse -Force -ErrorAction SilentlyContinue
$proc=Start-Process $exe -PassThru
$h=Wait-Until { $x=Get-WindowHandle;if($x -ne [IntPtr]::Zero){$x}else{$null}} 20000
Assert $h "startup" "Saeed started"

$log="$env:APPDATA\Saeed\logs\saeed.log"
$line=Wait-Until {
  if(Test-Path $log){
    $lines=Get-Content $log -ErrorAction SilentlyContinue
    $lines|Where-Object{$_ -match "Tray icon rect: x=(-?\d+) y=(-?\d+) width=(\d+) height=(\d+)"}|Select-Object -Last 1
  }
} 10000
Assert ($null -ne $line) "tray-native-geometry" "native Tauri tray rect was reported without UI Automation"
if($line){
  if($line -match "x=(-?\d+) y=(-?\d+) width=(\d+) height=(\d+)"){
    $x=[int]$Matches[1];$y=[int]$Matches[2];$w=[int]$Matches[3];$hh=[int]$Matches[4]
    Assert ($w -gt 0 -and $hh -gt 0) "tray-geometry-valid" "tray icon has a real screen rectangle"
    $tx=$x+[int]($w/2);$ty=$y+[int]($hh/2)

    ClickPoint $tx $ty $false
    $hidden=Wait-Until { if((Get-WindowHandle)-eq [IntPtr]::Zero){$true}else{$null}} 10000
    Assert $hidden "tray-left-click-hide" "native mouse click on the real tray rectangle hid/destroyed the character"

    ClickPoint $tx $ty $false
    $shown=Wait-Until { $z=Get-WindowHandle;if($z -ne [IntPtr]::Zero){$z}else{$null}} 10000
    Assert ($null -ne $shown) "tray-left-click-show" "native mouse click on the real tray rectangle recreated the character"

    ClickPoint $tx $ty $true
    $popup=Wait-Until { $p=Get-MenuPopup;if($p -ne [IntPtr]::Zero){$p}else{$null}} 3000
    Assert ($popup -ne $null) "tray-right-click-menu" "native right click opened a Windows popup menu without UI Automation"
    if($popup -ne $null){
      Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class TraySmokeKeys {
 [DllImport("user32.dll")] public static extern void keybd_event(byte bVk,byte bScan,uint dwFlags,UIntPtr dwExtraInfo);
 public const uint KEYUP=0x0002;
}
"@
      # Escape closes the native popup; no UI Automation is used.
      [TraySmokeKeys]::keybd_event(0x1B,0,0,[UIntPtr]::Zero)
      [TraySmokeKeys]::keybd_event(0x1B,0,[TraySmokeKeys]::KEYUP,[UIntPtr]::Zero)
      Start-Sleep -Milliseconds 300
    }


    # Invoke menu commands by real mouse clicks on the native popup menu.
    # This is intentionally not keyboard navigation: Phase 1 acceptance requires
    # clicking the tray icon and clicking the actual menu/submenu item.
function ClickMenuItem([IntPtr]$popup,[int]$index){
      $menu=[TraySmokeWin32]::GetMenu($popup)
      if($menu -eq [IntPtr]::Zero){return $false}
      $rr=New-Object TraySmokeWin32+RECT
      if(![TraySmokeWin32]::GetMenuItemRect($popup,$menu,[uint32]$index,[ref]$rr)){return $false}
      $cx=[int](($rr.Left+$rr.Right)/2); $cy=[int](($rr.Top+$rr.Bottom)/2)
      ClickPoint $cx $cy $false
      Start-Sleep -Milliseconds 300
      return $true
    }
    function OpenMenuNative {
      ClickPoint $tx $ty $true
      return (Wait-Until { $p=Get-MenuPopup; if($p -ne [IntPtr]::Zero){$p}else{$null}} 3000)
    }
    function HoverMenuItem([IntPtr]$popup,[int]$index){
      $menu=[TraySmokeWin32]::GetMenu($popup)
      if($menu -eq [IntPtr]::Zero){return $false}
      $rr=New-Object TraySmokeWin32+RECT
      if(![TraySmokeWin32]::GetMenuItemRect($popup,$menu,[uint32]$index,[ref]$rr)){return $false}
      $cx=[int](($rr.Left+$rr.Right)/2); $cy=[int](($rr.Top+$rr.Bottom)/2)
      [TraySmokeWin32]::SetCursorPos($cx,$cy)|Out-Null
      Start-Sleep -Milliseconds 500
      return $true
    }
    function InvokeMenuClick([int]$topIndex,[int]$submenuIndex=-1){
      $p=OpenMenuNative
      if($p -eq $null){return $false}
      if($submenuIndex -ge 0){
        if(-not (HoverMenuItem $p $topIndex)){return $false}
        $sub=Wait-Until {
          $fg=[TraySmokeWin32]::GetForegroundWindow()
          if($fg -ne [IntPtr]::Zero){
            $name=New-Object System.Text.StringBuilder 64
            [TraySmokeWin32]::GetClassName($fg,$name,$name.Capacity)|Out-Null
            if($name.ToString() -eq "#32768" -and $fg -ne $p){$fg}
          }
          $null
        } 3000
        if($sub -eq $null){return $false}
        return (ClickMenuItem $sub $submenuIndex)
      }
      return (ClickMenuItem $p $topIndex)
    }

    # Show/Hide: click the enabled item itself. The disabled counterpart is not clicked.
    Assert (InvokeMenuClick 1) "tray-menu-hide-command" "real mouse click selected Hide Saeed"
    $hidden=Wait-Until { if((Get-WindowHandle)-eq [IntPtr]::Zero){$true}else{$null}} 5000
    Assert $hidden "tray-menu-hide-applied" "Hide command destroyed the character window"

    Assert (InvokeMenuClick 0) "tray-menu-show-command" "real mouse click selected Show Saeed"
    $shown=Wait-Until { $z=Get-WindowHandle;if($z -ne [IntPtr]::Zero){$z}else{$null}} 5000
    Assert ($shown -ne $null) "tray-menu-show-applied" "Show command recreated the character window"

    # Character Size -> Small, Medium, Large
    $sizes=@{small=0;medium=1;large=2}
    foreach($size in @("small","medium","large")){
      $ok=InvokeMenuClick 3 $sizes[$size]
      Assert $ok "tray-size-$size-command" "real mouse click selected Character Size -> $size"
      $expected=@{small=280;medium=360;large=460}[$size]
      $ok=Wait-Until {
        $z=Get-WindowHandle; if($z -eq [IntPtr]::Zero){return $null}
        $rr=New-Object TraySmokeWin32+RECT
        if([TraySmokeWin32]::GetWindowRect($z,[ref]$rr)){ if(($rr.Right-$rr.Left) -eq $expected){$true}else{$null} }
      } 5000
      Assert $ok "tray-size-$size-applied" "Character Size -> $size applied the expected window size"
    }

    # Always on Top toggle twice.
    Assert (InvokeMenuClick 4) "tray-always-on-top-toggle-1" "real mouse click selected Always on Top"
    Assert (InvokeMenuClick 4) "tray-always-on-top-toggle-2" "real mouse click selected Always on Top again"

    # Low Power toggle twice.
    Assert (InvokeMenuClick 5) "tray-low-power-toggle-1" "real mouse click selected Low Power Mode"
    Assert (InvokeMenuClick 5) "tray-low-power-toggle-2" "real mouse click selected Low Power Mode again"

    # Debug -> Rotate once.
    Assert (InvokeMenuClick 6 0) "tray-rotate-once-command" "real mouse click selected Debug -> Rotate once"
    Assert ((Get-WindowHandle) -ne [IntPtr]::Zero) "tray-rotate-once-stable" "character remained alive after Rotate once"

    # Change Character -> native file dialog, then cancel.
    Assert (InvokeMenuClick 2) "tray-change-character-command" "real mouse click selected Change Character"
    Start-Sleep -Milliseconds 500
    $dialog=[TraySmokeWin32]::FindWindow("#32770",$null)
    Assert ($dialog -ne [IntPtr]::Zero) "tray-change-character-dialog" "Change Character opened the native file dialog"
if($dialog -ne [IntPtr]::Zero){ [TraySmokeKeys]::keybd_event(0x1B,0,0,[UIntPtr]::Zero); [TraySmokeKeys]::keybd_event(0x1B,0,[TraySmokeKeys]::KEYUP,[UIntPtr]::Zero) }

    # Quit is final and must terminate the application.
Assert (InvokeMenuClick 7) "tray-quit-command" "real mouse click selected Quit"
    $exited=Wait-Until { if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000
    Assert $exited "tray-quit" "Quit removed the Saeed process"
    Assert ((Get-SaeedProcess).Count -eq 0) "tray-process-lifetime" "Quit terminated the application"
  }
}

Get-SaeedProcess | Stop-Process -Force -ErrorAction SilentlyContinue
Wait-Until { if((Get-SaeedProcess).Count -eq 0){$true}else{$null}} 10000 | Out-Null
$results | ConvertTo-Json -Depth 4 | Tee-Object "$env:RUNNER_TEMP\phase1-tray-results.json"
$failed=@($results.GetEnumerator()|Where-Object{$_.Value -like "FAIL*"})
Write-Host "TRAY TEST SUMMARY: $($results.Count) checks, $($failed.Count) failed" -ForegroundColor $(if($failed.Count){"Red"}else{"Green"})
if($failed.Count){exit 1}
exit 0
