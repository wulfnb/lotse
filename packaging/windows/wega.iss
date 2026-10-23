; Per-user installer. It registers capabilities without changing UserChoice.
[Setup]
AppId=Wulfnb.Wega
AppName=Wega
AppVersion=0.1.0-alpha.1
DefaultDirName={localappdata}\Programs\Wega
DefaultGroupName=Wega
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=Wega-Windows-x64-Setup
UninstallDisplayIcon={app}\wega.exe
ChangesAssociations=yes
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "..\..\dist\wega-windows-x64\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Wega"; Filename: "{app}\wega.exe"
Name: "{group}\Uninstall Wega"; Filename: "{uninstallexe}"

[Registry]
Root: HKCU; Subkey: "Software\Classes\WegaHTML"; ValueType: string; ValueData: "Wega HTML document"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\WegaHTML\DefaultIcon"; ValueType: string; ValueData: "{app}\wega.exe,0"
Root: HKCU; Subkey: "Software\Classes\WegaHTML\shell\open\command"; ValueType: string; ValueData: """{app}\wega.exe"" -- ""%1"""
Root: HKCU; Subkey: "Software\Classes\WegaURL"; ValueType: string; ValueData: "Wega URL"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\WegaURL"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\WegaURL\DefaultIcon"; ValueType: string; ValueData: "{app}\wega.exe,0"
Root: HKCU; Subkey: "Software\Classes\WegaURL\shell\open\command"; ValueType: string; ValueData: """{app}\wega.exe"" -- ""%1"""
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega"; ValueType: string; ValueData: "Wega"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\DefaultIcon"; ValueType: string; ValueData: "{app}\wega.exe,0"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\shell\open\command"; ValueType: string; ValueData: """{app}\wega.exe"""
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Wega"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Wega web browser"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\Capabilities\URLAssociations"; ValueType: string; ValueName: "http"; ValueData: "WegaURL"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\Capabilities\URLAssociations"; ValueType: string; ValueName: "https"; ValueData: "WegaURL"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\Capabilities\FileAssociations"; ValueType: string; ValueName: ".html"; ValueData: "WegaHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\Wega\Capabilities\FileAssociations"; ValueType: string; ValueName: ".htm"; ValueData: "WegaHTML"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "Wega"; ValueData: "Software\Clients\StartMenuInternet\Wega\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\wega.exe"; Description: "Open Wega"; Flags: nowait postinstall skipifsilent
