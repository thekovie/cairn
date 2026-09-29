; Cairn installer (Inno Setup 6).
;
; Installs Cairn for the person running it, with no administrator password:
; into %LOCALAPPDATA%\Programs\Cairn, with a Start menu shortcut and an
; entry in Settings > Apps for uninstalling. Because that folder belongs to
; the person, Cairn can update itself there.
;
; Built by the release workflow:
;   ISCC /DAppVersion=0.5.0 /DSourceDir=<folder with cairn.exe> /O<output folder> installer\cairn.iss
;
; Uninstalling leaves the person's settings and unsaved changes
; (%LOCALAPPDATA%\Cairn) and every documentation folder untouched.

#ifndef AppVersion
  #error Pass /DAppVersion=x.y.z
#endif
#ifndef SourceDir
  #error Pass /DSourceDir=<folder containing cairn.exe>
#endif

[Setup]
AppId={{8C1B6E0B-2F4B-4E7A-9C2D-6A1E5B3F7D10}
AppName=Cairn
AppVersion={#AppVersion}
AppVerName=Cairn {#AppVersion}
AppPublisher=Cairn
AppPublisherURL=https://github.com/thekovie/cairn
AppSupportURL=https://github.com/thekovie/cairn/issues
AppUpdatesURL=https://github.com/thekovie/cairn/releases
DefaultDirName={localappdata}\Programs\Cairn
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
UsedUserAreasWarning=no
OutputBaseFilename=cairn-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\cairn.exe
UninstallDisplayName=Cairn
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Put a Cairn shortcut on the desktop"; Flags: unchecked

[Files]
Source: "{#SourceDir}\cairn.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Cairn"; Filename: "{app}\cairn.exe"; Comment: "Shared documentation"
Name: "{autodesktop}\Cairn"; Filename: "{app}\cairn.exe"; Comment: "Shared documentation"; Tasks: desktopicon

[Run]
Filename: "{app}\cairn.exe"; Description: "Start Cairn now"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Files Cairn's own updates leave beside it.
Type: files; Name: "{app}\cairn.previous.exe"
Type: files; Name: "{app}\cairn.previous.version"
Type: files; Name: "{app}\cairn-*.new.exe"
Type: files; Name: "{app}\cairn-*.swap.exe"
Type: files; Name: "{app}\.cairn*"
