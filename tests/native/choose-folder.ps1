param([int]$AppProcessId, [string]$Folder)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class PickerInput {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, IntPtr wParam, string text, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
}
'@
$taskRoot = [System.Windows.Automation.AutomationElement]::RootElement
$taskFilter = [System.Windows.Automation.AndCondition]::new(
    [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $AppProcessId),
    [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, 'Choose the Sanctuary installation folder'))
$taskDeadline = [DateTime]::UtcNow.AddSeconds(10)
do {
    $taskWindow = $taskRoot.FindFirst([System.Windows.Automation.TreeScope]::Children, $taskFilter)
    if ($taskWindow) { break }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $taskDeadline)
if (!$taskWindow) { throw 'The test app did not open its folder picker.' }
if (!$Folder) {
    $taskWindow.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
    exit
}
$taskControls = $taskWindow.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$taskEdit = $taskControls | Where-Object { $_.Current.AutomationId -eq '1152' -and $_.Current.NativeWindowHandle -ne 0 } | Select-Object -First 1
$taskSelect = $taskControls | Where-Object { $_.Current.AutomationId -eq '1' -and $_.Current.NativeWindowHandle -ne 0 } | Select-Object -First 1
if (!$taskEdit -or !$taskSelect) { throw 'The native folder controls were not found.' }
$taskResult = [IntPtr]::Zero
if ([PickerInput]::SendMessageTimeout([IntPtr]$taskEdit.Current.NativeWindowHandle, 0x000C, [IntPtr]::Zero, $Folder, 2, 3000, [ref]$taskResult) -eq [IntPtr]::Zero) { throw 'The folder field did not respond.' }
if (![PickerInput]::PostMessage([IntPtr]$taskSelect.Current.NativeWindowHandle, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)) { throw 'The Select Folder button did not respond.' }
