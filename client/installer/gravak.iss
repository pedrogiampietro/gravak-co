; Gravak installer (Inno Setup 6). Built by tools\package-client.ps1 from
; dist\Gravak; it can also be compiled by hand once dist\Gravak exists:
;   ISCC.exe installer\gravak.iss

#define AppName "Gravak"
#define AppVersion "0.1.0"
#define AppExe "Gravak.exe"

[Setup]
AppId={{6E9C3A57-4C1B-4F5E-9B67-2D7A1E0F8C41}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Gravak
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
; Installed per user: no administrator prompt, and the client can write its
; settings beside itself.
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=GravakSetup
SetupIconFile=gravak.ico
UninstallDisplayIcon={app}\{#AppExe}
; ultra64's dictionary (over 1 GB) is more than the 32-bit compiler can
; hold for 1.2 GB of game data; ultra is nearly as small. The separate
; (64-bit) process gives the compressor the memory it needs.
Compression=lzma2/ultra
SolidCompression=yes
LZMAUseSeparateProcess=yes
LZMANumBlockThreads=2
WizardStyle=modern
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\dist\Gravak\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; What the client writes at run time (options, hotkeys, last account).
Type: filesandordirs; Name: "{app}\settings"
