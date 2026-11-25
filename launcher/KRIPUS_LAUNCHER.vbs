Set WshShell = CreateObject("WScript.Shell")
Set Fso = CreateObject("Scripting.FileSystemObject")
LauncherDir = Fso.GetParentFolderName(WScript.ScriptFullName)
WshShell.CurrentDirectory = LauncherDir
WshShell.Run "powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File """ & Fso.BuildPath(LauncherDir, "kripus_launcher.ps1") & """", 0, False
