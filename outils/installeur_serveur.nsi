; Installeur de Gescom Serveur — a part de la fenetre, avec les droits
; administrateur que celle-ci n'a pas (installMode currentUser).
;
; Ce qu'il fait, dans l'ordre :
;   1. copie gescom-serveur.exe dans Program Files ;
;   2. enregistre le certificat auto-signe comme editeur de confiance
;      de CETTE machine (les installations suivantes ne demandent plus
;      rien ici — ailleurs, l'avertissement reste : D5) ;
;   3. ecrit la configuration (base, port) dans ProgramData\Gescom, sans
;      ecraser une configuration existante ;
;   4. ouvre le port dans le pare-feu (profils Domaine et Prive, sur le
;      PORT, comme outils\parefeu.ps1) ;
;   5. installe et demarre le service Windows « GescomServeur ».
; La desinstallation defait les cinq, et garde la base et les
; sauvegardes : un commerçant qui desinstalle ne veut pas perdre ses
; ventes.
;
; Construit par outils\construire_installeur_serveur.ps1, qui passe
; VERSION, SOURCE (le dossier des fichiers) et SORTIE.

Unicode True
!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SOURCE
  !define SOURCE "dist"
!endif
!ifndef SORTIE
  !define SORTIE "Gescom-Serveur_${VERSION}_x64-setup.exe"
!endif

!define NOM "Gescom Serveur"
!define SERVICE "GescomServeur"
!define REGLE_PAREFEU "Gescom serveur"
!define CLE_DESINSTALL "Software\Microsoft\Windows\CurrentVersion\Uninstall\GescomServeur"

Name "${NOM} ${VERSION}"
OutFile "${SORTIE}"
InstallDir "$PROGRAMFILES64\Gescom Serveur"
InstallDirRegKey HKLM "Software\Gescom\Serveur" "Dossier"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "Gescom — ${VERSION}"

Var BaseTexte
Var PortTexte
Var ChampBase
Var ChampPort
Var DossierConfig

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
Page custom PageConfiguration PageConfigurationValider
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "Le service « ${NOM} » tourne et demarre avec la machine.$\r$\n$\r$\nJournal : $DossierConfig\serveur.log$\r$\nConfiguration : $DossierConfig\serveur.json$\r$\n$\r$\nL'arreter / le relancer : sc stop ${SERVICE} / sc start ${SERVICE}"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "French"

; ---------------------------------------------------------------------
;  La page de configuration : ou est la base, sur quel port ecouter
; ---------------------------------------------------------------------

Function .onInit
  StrCpy $DossierConfig "$%ProgramData%\Gescom"
  ; Une base SQLite dans ProgramData par defaut : le service tourne sous
  ; LocalSystem, dont le profil n'est pas un endroit pour une boutique.
  ; Barres obliques : JSON n'aime pas les antislashs, Rust lit les deux.
  StrCpy $BaseTexte "$DossierConfig\gescom.db"
  StrCpy $PortTexte "7300"
FunctionEnd

Function PageConfiguration
  ; Une configuration deja en place se garde : cette page ne s'affiche
  ; qu'a la premiere installation. Pour la changer, editer serveur.json
  ; et relancer le service.
  ${If} ${FileExists} "$DossierConfig\serveur.json"
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Configuration du serveur" "Ou est la base, et sur quel port ecouter."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 24u "Base de donnees : un chemin de fichier SQLite, ou une adresse PostgreSQL de la forme$\r$\npostgresql://utilisateur:motdepasse@127.0.0.1:5432/gescom"
  Pop $0
  ${NSD_CreateText} 0 28u 100% 12u "$BaseTexte"
  Pop $ChampBase
  ${NSD_CreateLabel} 0 50u 100% 12u "Port d'ecoute (7300 par defaut, sur toutes les adresses de la machine) :"
  Pop $0
  ${NSD_CreateNumber} 0 64u 60u 12u "$PortTexte"
  Pop $ChampPort
  ${NSD_CreateLabel} 0 86u 100% 36u "Le mot de passe PostgreSQL restera dans $DossierConfig\serveur.json, lisible par les administrateurs de cette machine seulement. Il n'est envoye nulle part."
  Pop $0
  nsDialogs::Show
FunctionEnd

Function PageConfigurationValider
  ${NSD_GetText} $ChampBase $BaseTexte
  ${NSD_GetText} $ChampPort $PortTexte
  ${If} $BaseTexte == ""
    MessageBox MB_ICONEXCLAMATION|MB_OK "Indiquer une base."
    Abort
  ${EndIf}
  ${If} $PortTexte == ""
    StrCpy $PortTexte "7300"
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------
;  Installation
; ---------------------------------------------------------------------

Section "Serveur" SEC_SERVEUR
  SectionIn RO
  SetOutPath "$INSTDIR"

  ; Mise a jour : l'ancien service tient l'executable ; on le retire
  ; d'abord, sinon la copie echoue. Sans service en place, ces deux
  ; commandes echouent sans consequence.
  ${If} ${FileExists} "$INSTDIR\gescom-serveur.exe"
    nsExec::ExecToLog 'sc.exe stop ${SERVICE}'
    Sleep 2000
    nsExec::ExecToLog '"$INSTDIR\gescom-serveur.exe" --desinstaller-service'
    Sleep 1000
  ${EndIf}

  File "${SOURCE}\gescom-serveur.exe"
  File "${SOURCE}\gescom.cer"

  ; 2. Le certificat, editeur de confiance de cette machine.
  nsExec::ExecToLog 'certutil.exe -addstore -f Root "$INSTDIR\gescom.cer"'
  nsExec::ExecToLog 'certutil.exe -addstore -f TrustedPublisher "$INSTDIR\gescom.cer"'

  ; 3. La configuration — jamais par-dessus une existante.
  CreateDirectory "$DossierConfig"
  ${IfNot} ${FileExists} "$DossierConfig\serveur.json"
    Push $BaseTexte
    Call EchapperJson
    Pop $1
    FileOpen $0 "$DossierConfig\serveur.json" w
    FileWrite $0 '{$\r$\n  "base": "$1",$\r$\n  "port": $PortTexte$\r$\n}$\r$\n'
    FileClose $0
  ${EndIf}

  ; 4. Le pare-feu : sur le port, profils Domaine et Prive (pas Public :
  ;    le reseau d'un hotel n'a pas a voir la base d'un commerce).
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${REGLE_PAREFEU}"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${REGLE_PAREFEU}" dir=in action=allow protocol=TCP localport=$PortTexte profile=domain,private'

  ; 5. Le service.
  nsExec::ExecToLog '"$INSTDIR\gescom-serveur.exe" --installer-service'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "Le service n'a pas pu etre installe (code $0). Voir le detail ci-dessus ; on peut relancer plus tard, en administrateur :$\r$\n$\r$\n$\"$INSTDIR\gescom-serveur.exe$\" --installer-service"
  ${EndIf}

  ; Desinstallation propre.
  WriteRegStr HKLM "Software\Gescom\Serveur" "Dossier" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Desinstaller.exe"
  WriteRegStr HKLM "${CLE_DESINSTALL}" "DisplayName" "${NOM}"
  WriteRegStr HKLM "${CLE_DESINSTALL}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${CLE_DESINSTALL}" "Publisher" "Gescom"
  WriteRegStr HKLM "${CLE_DESINSTALL}" "UninstallString" '"$INSTDIR\Desinstaller.exe"'
  WriteRegStr HKLM "${CLE_DESINSTALL}" "DisplayIcon" '"$INSTDIR\gescom-serveur.exe"'
  WriteRegDWORD HKLM "${CLE_DESINSTALL}" "NoModify" 1
  WriteRegDWORD HKLM "${CLE_DESINSTALL}" "NoRepair" 1

  CreateDirectory "$SMPROGRAMS\Gescom"
  CreateShortcut "$SMPROGRAMS\Gescom\Journal du serveur.lnk" "notepad.exe" '"$DossierConfig\serveur.log"'
  CreateShortcut "$SMPROGRAMS\Gescom\Configuration du serveur.lnk" "notepad.exe" '"$DossierConfig\serveur.json"'
  CreateShortcut "$SMPROGRAMS\Gescom\Console du serveur.lnk" "http://localhost:$PortTexte"
SectionEnd

; Un antislash dans une chaine JSON doit etre double.
Function EchapperJson
  Exch $0
  Push $1
  Push $2
  Push $3
  StrCpy $1 ""
  StrCpy $2 0
  boucle:
    StrCpy $3 $0 1 $2
    StrCmp $3 "" fin
    StrCmp $3 "\" 0 +3
      StrCpy $1 "$1\\"
      Goto suite
    StrCpy $1 "$1$3"
    suite:
    IntOp $2 $2 + 1
    Goto boucle
  fin:
  StrCpy $0 $1
  Pop $3
  Pop $2
  Pop $1
  Exch $0
FunctionEnd

; ---------------------------------------------------------------------
;  Desinstallation — le service, le pare-feu, les fichiers du programme.
;  La base, les sauvegardes et le journal restent dans ProgramData.
; ---------------------------------------------------------------------

Section "Uninstall"
  StrCpy $DossierConfig "$%ProgramData%\Gescom"
  nsExec::ExecToLog 'sc.exe stop ${SERVICE}'
  Sleep 2000
  nsExec::ExecToLog '"$INSTDIR\gescom-serveur.exe" --desinstaller-service'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${REGLE_PAREFEU}"'
  Delete "$INSTDIR\gescom-serveur.exe"
  Delete "$INSTDIR\gescom.cer"
  Delete "$INSTDIR\Desinstaller.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Gescom\Journal du serveur.lnk"
  Delete "$SMPROGRAMS\Gescom\Configuration du serveur.lnk"
  Delete "$SMPROGRAMS\Gescom\Console du serveur.lnk"
  RMDir "$SMPROGRAMS\Gescom"
  DeleteRegKey HKLM "${CLE_DESINSTALL}"
  DeleteRegKey HKLM "Software\Gescom\Serveur"
  MessageBox MB_ICONINFORMATION|MB_OK "Le service est retire. La base, les sauvegardes et le journal sont conserves dans$\r$\n$DossierConfig"
SectionEnd
