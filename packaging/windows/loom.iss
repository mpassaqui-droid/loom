; Installateur Windows de LOOM (Inno Setup 6). Version passée par /DVersion=…
#ifndef Version
  #define Version "0.0.0"
#endif
[Setup]
AppId={{8C1F7A52-3B1E-4E0B-9F43-6A0C2B7D5E11}
AppName=LOOM
AppVersion={#Version}
AppPublisher=Munay Passaqui
AppPublisherURL=https://munaypassaqui.com/loom/
DefaultDirName={autopf}\LOOM
DefaultGroupName=LOOM
OutputBaseFilename=LOOM-{#Version}-windows-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
PrivilegesRequiredOverridesAllowed=dialog
UninstallDisplayName=LOOM

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\..\target\release\loom.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; LOOM est une app de terminal : le raccourci l'ouvre dans une console
Name: "{group}\LOOM"; Filename: "{app}\loom.exe"
Name: "{autodesktop}\LOOM"; Filename: "{app}\loom.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked
Name: "path"; Description: "Ajouter la commande loom au PATH"

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: path; Check: NeedsPath(ExpandConstant('{app}'))

[Code]
function NeedsPath(Dir: string): Boolean;
var Cur: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Cur) then Cur := '';
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Cur) + ';') = 0;
end;
