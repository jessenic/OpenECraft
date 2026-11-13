; OpenECraft setup for Windows (Inno Setup 6).
;
; Built by installer/windows/build.ps1, which compiles the release binaries and passes
; the version and paths:  iscc /DAppVersion=0.1.0 /DBinDir=<release dir> openecraft.iss
;
; Installs per user without administrator rights; "Install for all users" in the first
; dialog installs to Program Files. Silent installation: OpenECraft-<version>-setup.exe
; /VERYSILENT /SUPPRESSMSGBOXES [/ALLUSERS] [/COMPONENTS="main,cli"] [/TASKS="fileassoc"]

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef BinDir
  #define BinDir "..\..\target\x86_64-pc-windows-msvc\release"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\target\installer"
#endif
#ifndef Notices
  #define Notices "..\..\target\installer\windows\THIRD-PARTY-LICENSES.html"
#endif

[Setup]
AppId={{53B40BA8-0D7F-4081-8069-C7A181DEFA3C}
AppName=OpenECraft
AppVersion={#AppVersion}
AppVerName=OpenECraft {#AppVersion}
AppPublisher=OpenECraft
AppPublisherURL=https://github.com/nilres/OpenECraft
AppSupportURL=https://github.com/nilres/OpenECraft/issues
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\OpenECraft
DefaultGroupName=OpenECraft
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=OpenECraft-{#AppVersion}-setup
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\openecraft.ico
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
ChangesAssociations=yes
ChangesEnvironment=yes
#ifdef SignTool
SignTool=signtool
SignedUninstaller=yes
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[CustomMessages]
en.TypeEditor=Editor
en.TypeFull=Full installation
en.TypeCustom=Custom installation
en.ComponentEditor=OpenECraft editor
en.ComponentPartsManager=Parts manager (maintains the parts of a parts server)
en.ComponentCli=Command line and MCP server for AI agents
en.ComponentServers=Parts server and project server
en.Integration=Windows integration:
en.AssocProjects=Open OpenECraft projects (.oec, .oecz, .oecp) with OpenECraft
en.AddToPath=Add OpenECraft to the PATH (command line)
en.ProjectFile=OpenECraft project
en.ArchiveFile=OpenECraft project archive
en.SplitFile=OpenECraft split project
en.PartsManager=OpenECraft parts manager
de.TypeEditor=Editor
de.TypeFull=Vollständige Installation
de.TypeCustom=Benutzerdefinierte Installation
de.ComponentEditor=OpenECraft-Editor
de.ComponentPartsManager=Artikelverwaltung (pflegt die Artikel eines Artikelservers)
de.ComponentCli=Kommandozeile und MCP-Server für KI-Agenten
de.ComponentServers=Artikelserver und Projektserver
de.Integration=Windows-Integration:
de.AssocProjects=OpenECraft-Projekte (.oec, .oecz, .oecp) mit OpenECraft öffnen
de.AddToPath=OpenECraft zum PATH hinzufügen (Kommandozeile)
de.ProjectFile=OpenECraft-Projekt
de.ArchiveFile=OpenECraft-Projektarchiv
de.SplitFile=Geteiltes OpenECraft-Projekt
de.PartsManager=OpenECraft-Artikelverwaltung
fr.TypeEditor=Éditeur
fr.TypeFull=Installation complète
fr.TypeCustom=Installation personnalisée
fr.ComponentEditor=Éditeur OpenECraft
fr.ComponentPartsManager=Gestionnaire d'articles (gère les articles d'un serveur d'articles)
fr.ComponentCli=Ligne de commande et serveur MCP pour les agents IA
fr.ComponentServers=Serveur d'articles et serveur de projets
fr.Integration=Intégration à Windows :
fr.AssocProjects=Ouvrir les projets OpenECraft (.oec, .oecz, .oecp) avec OpenECraft
fr.AddToPath=Ajouter OpenECraft au PATH (ligne de commande)
fr.ProjectFile=Projet OpenECraft
fr.ArchiveFile=Archive de projet OpenECraft
fr.SplitFile=Projet fractionné OpenECraft
fr.PartsManager=Gestionnaire d'articles OpenECraft
es.TypeEditor=Editor
es.TypeFull=Instalación completa
es.TypeCustom=Instalación personalizada
es.ComponentEditor=Editor OpenECraft
es.ComponentPartsManager=Gestor de artículos (mantiene los artículos de un servidor de artículos)
es.ComponentCli=Línea de comandos y servidor MCP para agentes de IA
es.ComponentServers=Servidor de artículos y servidor de proyectos
es.Integration=Integración con Windows:
es.AssocProjects=Abrir los proyectos de OpenECraft (.oec, .oecz, .oecp) con OpenECraft
es.AddToPath=Añadir OpenECraft al PATH (línea de comandos)
es.ProjectFile=Proyecto de OpenECraft
es.ArchiveFile=Archivo comprimido de proyecto OpenECraft
es.SplitFile=Proyecto dividido de OpenECraft
es.PartsManager=Gestor de artículos de OpenECraft

[Types]
Name: "editor"; Description: "{cm:TypeEditor}"
Name: "full"; Description: "{cm:TypeFull}"
Name: "custom"; Description: "{cm:TypeCustom}"; Flags: iscustom

[Components]
Name: "main"; Description: "{cm:ComponentEditor}"; Types: editor full custom; Flags: fixed
Name: "partsmanager"; Description: "{cm:ComponentPartsManager}"; Types: full
Name: "cli"; Description: "{cm:ComponentCli}"; Types: full
Name: "servers"; Description: "{cm:ComponentServers}"; Types: full

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "fileassoc"; Description: "{cm:AssocProjects}"; GroupDescription: "{cm:Integration}"
Name: "addtopath"; Description: "{cm:AddToPath}"; GroupDescription: "{cm:Integration}"; Components: cli

[Files]
Source: "{#BinDir}\openecraft.exe"; DestDir: "{app}"; Components: main; Flags: ignoreversion
Source: "..\..\assets\icon.ico"; DestDir: "{app}"; DestName: "openecraft.ico"; Components: main; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}\licenses"; Components: main; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}\licenses"; Components: main; Flags: ignoreversion
Source: "..\..\crates\openecraft-fonts\fonts\OFL.txt"; DestDir: "{app}\licenses"; DestName: "OFL-NotoSans.txt"; Components: main; Flags: ignoreversion
Source: "{#Notices}"; DestDir: "{app}\licenses"; Components: main; Flags: ignoreversion
Source: "{#BinDir}\openecraft-parts-manager.exe"; DestDir: "{app}"; Components: partsmanager; Flags: ignoreversion
Source: "{#BinDir}\openecraft-cli.exe"; DestDir: "{app}"; Components: cli; Flags: ignoreversion
Source: "{#BinDir}\openecraft-parts-server.exe"; DestDir: "{app}"; Components: servers; Flags: ignoreversion
Source: "{#BinDir}\openecraft-project-server.exe"; DestDir: "{app}"; Components: servers; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\OpenECraft"; Filename: "{app}\openecraft.exe"; IconFilename: "{app}\openecraft.ico"; Components: main
Name: "{autoprograms}\{cm:PartsManager}"; Filename: "{app}\openecraft-parts-manager.exe"; IconFilename: "{app}\openecraft.ico"; Components: partsmanager
Name: "{autodesktop}\OpenECraft"; Filename: "{app}\openecraft.exe"; IconFilename: "{app}\openecraft.ico"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.oec"; ValueType: string; ValueName: ""; ValueData: "OpenECraft.Project"; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.oec\OpenWithProgids"; ValueType: string; ValueName: "OpenECraft.Project"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.Project"; ValueType: string; ValueName: ""; ValueData: "{cm:ProjectFile}"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\openecraft.ico"; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\openecraft.exe"" ""%1"""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.oecz"; ValueType: string; ValueName: ""; ValueData: "OpenECraft.Archive"; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.oecz\OpenWithProgids"; ValueType: string; ValueName: "OpenECraft.Archive"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.Archive"; ValueType: string; ValueName: ""; ValueData: "{cm:ArchiveFile}"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.Archive\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\openecraft.ico"; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.Archive\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\openecraft.exe"" ""%1"""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.oecp"; ValueType: string; ValueName: ""; ValueData: "OpenECraft.SplitProject"; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.oecp\OpenWithProgids"; ValueType: string; ValueName: "OpenECraft.SplitProject"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.SplitProject"; ValueType: string; ValueName: ""; ValueData: "{cm:SplitFile}"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.SplitProject\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\openecraft.ico"; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\OpenECraft.SplitProject\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\openecraft.exe"" ""%1"""; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Applications\openecraft.exe"; Flags: uninsdeletekey; Components: main
Root: HKA; Subkey: "Software\Classes\Applications\openecraft.exe\SupportedTypes"; ValueType: string; ValueName: ".oec"; ValueData: ""; Components: main
Root: HKA; Subkey: "Software\Classes\Applications\openecraft.exe\SupportedTypes"; ValueType: string; ValueName: ".oecz"; ValueData: ""; Components: main
Root: HKA; Subkey: "Software\Classes\Applications\openecraft.exe\SupportedTypes"; ValueType: string; ValueName: ".oecp"; ValueData: ""; Components: main

[Run]
Filename: "{app}\openecraft.exe"; Description: "{cm:LaunchProgram,OpenECraft}"; Flags: nowait postinstall skipifsilent
; The update of the editor runs the setup silently and with /RESTART=1 starts it again.
Filename: "{app}\openecraft.exe"; Flags: nowait runasoriginaluser; Check: RestartAfterUpdate

[Code]
// The editor asked to be started again after its update (Help › Restart and update).
function RestartAfterUpdate: Boolean;
begin
  Result := WizardSilent and (ExpandConstant('{param:RESTART|0}') = '1');
end;

// The PATH of the user (per user installation) or of the machine (all users).
function EnvironmentRoot: Integer;
begin
  if IsAdminInstallMode then
    Result := HKEY_LOCAL_MACHINE
  else
    Result := HKEY_CURRENT_USER;
end;

function EnvironmentKey: String;
begin
  if IsAdminInstallMode then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

procedure AddToPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Paths) then
    Paths := '';
  if Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0 then
    exit;
  if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then
    Paths := Paths + ';';
  RegWriteExpandStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Paths + Dir);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths: String;
  P: Integer;
begin
  if not RegQueryStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Paths) then
    exit;
  Paths := ';' + Paths + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Paths));
  if P = 0 then
    exit;
  Delete(Paths, P, Length(Dir) + 1);
  RegWriteExpandStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Copy(Paths, 2, Length(Paths) - 2));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
