param([Parameter(Mandatory=$true)][int]$AppProcessId,[Parameter(Mandatory=$true)][ValidateSet('drag','inspect','restore')][string]$Action,[int]$ClientX=0,[int]$ClientY=0)
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class NativeDragCheck {
 public delegate bool Callback(IntPtr h,IntPtr arg);
 [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool EnumWindows(Callback cb,IntPtr arg);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder text,int size);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect rect);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref Point point);
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int cx,int cy,uint flags);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int command);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point point);
 [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll")] public static extern short GetAsyncKeyState(int key);
}
'@
[void][NativeDragCheck]::SetThreadDpiAwarenessContext([IntPtr](-4))
$ownedWindows=[System.Collections.Generic.List[IntPtr]]::new()
[NativeDragCheck]::EnumWindows({param($windowHandle,$argument) [uint32]$ownerId=0;[void][NativeDragCheck]::GetWindowThreadProcessId($windowHandle,[ref]$ownerId);if($ownerId -eq $AppProcessId){$title=[System.Text.StringBuilder]::new(256);[void][NativeDragCheck]::GetWindowText($windowHandle,$title,256);if($title.ToString().StartsWith('CortexOS')){$ownedWindows.Add($windowHandle)}};return $true},[IntPtr]::Zero) | Out-Null
if($ownedWindows.Count -ne 1){throw 'Expected exactly one owned CortexOS main window'}
$handle=$ownedWindows[0]
if($Action -eq 'restore'){[void][NativeDragCheck]::ShowWindow($handle,9);Write-Output '{"restored":true}';exit}
if($Action -eq 'inspect'){[pscustomobject]@{Minimized=[NativeDragCheck]::IsIconic($handle)}|ConvertTo-Json -Compress;exit}
if(([NativeDragCheck]::GetAsyncKeyState(1) -band 0x8000) -ne 0){throw 'Mouse button already held; native interaction skipped'}
$before=New-Object NativeDragCheck+Rect
$after=New-Object NativeDragCheck+Rect
$cursor=New-Object NativeDragCheck+Point
$point=New-Object NativeDragCheck+Point
$point.X=$ClientX;$point.Y=$ClientY
if(-not [NativeDragCheck]::GetWindowRect($handle,[ref]$before)){throw 'Cannot read owned window position'}
if(-not [NativeDragCheck]::GetCursorPos([ref]$cursor)){throw 'Cannot save cursor position'}
if(-not [NativeDragCheck]::ClientToScreen($handle,[ref]$point)){throw 'Cannot map client coordinates'}
try {
 # Only this disposable test window is temporarily raised; verify the hit before pressing the mouse.
 [void][NativeDragCheck]::ShowWindow($handle,9)
 [void][NativeDragCheck]::SetForegroundWindow($handle)
 [void][NativeDragCheck]::SetWindowPos($handle,[IntPtr](-1),0,0,0,0,0x0053)
 Start-Sleep -Milliseconds 100
 [void][NativeDragCheck]::SetCursorPos($point.X,$point.Y)
 [uint32]$hitProcessId=0
 [void][NativeDragCheck]::GetWindowThreadProcessId([NativeDragCheck]::GetAncestor([NativeDragCheck]::WindowFromPoint($point),2),[ref]$hitProcessId)
 if($hitProcessId -ne $AppProcessId){throw "Target point not owned: expected=$AppProcessId actual=$hitProcessId visible=$([NativeDragCheck]::IsWindowVisible($handle)) client=$ClientX,$ClientY screen=$($point.X),$($point.Y) window=$($before.Left),$($before.Top),$($before.Right),$($before.Bottom)"}
 [NativeDragCheck]::mouse_event(0x0002,0,0,0,[UIntPtr]::Zero)
 Start-Sleep -Milliseconds 300
 for($step=1;$step -le 4;$step++){[void][NativeDragCheck]::SetCursorPos($point.X+30*$step,$point.Y+12*$step);Start-Sleep -Milliseconds 45}
 [NativeDragCheck]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
 Start-Sleep -Milliseconds 100
 if(-not [NativeDragCheck]::GetWindowRect($handle,[ref]$after)){throw 'Cannot read moved window position'}
 [pscustomobject]@{DeltaX=$after.Left-$before.Left;DeltaY=$after.Top-$before.Top;WidthChanged=($after.Right-$after.Left)-($before.Right-$before.Left);HeightChanged=($after.Bottom-$after.Top)-($before.Bottom-$before.Top)}|ConvertTo-Json -Compress
}finally{
 [NativeDragCheck]::mouse_event(0x0004,0,0,0,[UIntPtr]::Zero)
 [void][NativeDragCheck]::SetWindowPos($handle,[IntPtr](-2),$before.Left,$before.Top,0,0,0x0011)
 [void][NativeDragCheck]::SetCursorPos($cursor.X,$cursor.Y)
}
