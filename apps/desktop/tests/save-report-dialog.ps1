param([Parameter(Mandatory=$true)][int]$AppProcessId,[string]$OutputPath,[switch]$Cancel)
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;using System.Text;using System.Runtime.InteropServices;
public static class ReportDialogWindow {
 public delegate bool Callback(IntPtr h,IntPtr arg);
 [DllImport("user32.dll")]public static extern bool EnumWindows(Callback cb,IntPtr arg);
 [DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetWindowText(IntPtr h,StringBuilder text,int size);
 [DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int command);
 [DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int hgt,uint flags);
}
'@
[void][ReportDialogWindow]::EnumWindows({param($handle,$argument) [uint32]$owner=0;[void][ReportDialogWindow]::GetWindowThreadProcessId($handle,[ref]$owner);if($owner -eq $AppProcessId){$title=[System.Text.StringBuilder]::new(256);[void][ReportDialogWindow]::GetWindowText($handle,$title,256);if($title.ToString().StartsWith('CortexOS')){[void][ReportDialogWindow]::ShowWindow($handle,9)}};return $true},[IntPtr]::Zero)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$condition=[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$AppProcessId)
$dialog=$null
$deadline=[DateTime]::UtcNow.AddSeconds(15)
while([DateTime]::UtcNow -lt $deadline){
 $windows=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$condition)
 foreach($window in $windows){if($window.Current.Name -eq '保存评测报告'){$dialog=$window;break}}
 if($null -ne $dialog){break}
 Start-Sleep -Milliseconds 100
}
if($null -eq $dialog){$names=@($windows | ForEach-Object {$_.Current.Name}) -join ', ';throw "The owned save-report window was not found; owned window titles: $names"}
[void][ReportDialogWindow]::ShowWindow([IntPtr]$dialog.Current.NativeWindowHandle,9)
[void][ReportDialogWindow]::SetWindowPos([IntPtr]$dialog.Current.NativeWindowHandle,[IntPtr]::Zero,100,80,960,680,4)
[void][ReportDialogWindow]::SetForegroundWindow([IntPtr]$dialog.Current.NativeWindowHandle)
if($Cancel){
 $buttons=$dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button))
 $button=$buttons | Where-Object {$_.Current.AutomationId -eq '2' -or $_.Current.Name -match '^(取消|Cancel)'} | Select-Object -First 1
 if($null -eq $button){throw 'Cancel button was not found'}
 $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
 Write-Output 'Owned save dialog cancelled'
 exit 0
}
if([string]::IsNullOrWhiteSpace($OutputPath)){throw 'An explicit test output path is required'}
$resolved=[System.IO.Path]::GetFullPath($OutputPath)
$workspace=[System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))+[System.IO.Path]::DirectorySeparatorChar
if(-not $resolved.StartsWith($workspace,[StringComparison]::OrdinalIgnoreCase)){throw 'Output must stay in this repository'}
$edit=$null
$deadline=[DateTime]::UtcNow.AddSeconds(8)
while([DateTime]::UtcNow -lt $deadline){
 $fields=$dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.OrCondition]::new([System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Edit),[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::ComboBox)))
 foreach($candidate in $fields){
  if($candidate.Current.AutomationId -in @('1001','1148','FileNameControlHost') -or $candidate.Current.Name -match '^(文件名|File name)'){
   $valuePattern=$null
   if($candidate.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$valuePattern) -and -not $valuePattern.Current.IsReadOnly){$edit=$candidate;break}
   $child=$candidate.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Edit))
   if($null -ne $child){$edit=$child;break}
  }
 }
 if($null -ne $edit){break}
 Start-Sleep -Milliseconds 100
}
if($null -eq $edit){throw 'The owned filename control is unavailable'}
$edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($resolved)
$buttons=$dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button))
$button=$buttons | Where-Object {$_.Current.AutomationId -eq '1' -or $_.Current.Name -match '^(保存|Save)'} | Select-Object -First 1
if($null -eq $button){throw 'Save button was not found'}
$button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
Write-Output 'Owned save dialog submitted'
