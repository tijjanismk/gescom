; Installeur de Gescom Serveur — a part de la fenetre, avec les droits
; administrateur que celle-ci n'a pas (installMode currentUser).
;
; Ce qu'il fait, dans l'ordre :
;   0. exige que Gescom (la caisse) soit ferme sur ce poste, et arrete tout
;      gescom-serveur.exe encore lance (D54) ;
;   1. copie gescom-serveur.exe et la fenetre gescom-serveur-fenetre.exe
;      (« Gescom Serveur », D25) dans Program Files ;
;   2. enregistre le certificat auto-signe comme editeur de confiance
;      de CETTE machine (les installations suivantes ne demandent plus
;      rien ici — ailleurs, l'avertissement reste : D5) ;
;   3. ecrit la configuration (base, port) dans ProgramData\Gescom, sans
;      ecraser une configuration existante ;
;   4. ouvre le pare-feu (profils Domaine et Prive, comme
;      outils\parefeu.ps1) : TCP sur la plage du serveur, et UDP 40000
;      pour que les caisses le trouvent (« Rechercher le serveur ») ;
;   4b. un poste qui travaillait seul : propose de copier sa base dans
;      celle du service, si celle-ci est neuve (D54) ;
;   5. installe et demarre le service Windows « GescomServeur » ;
;   6. pose au menu Demarrer « Gescom Serveur » (la fenetre), a cote de
;      la console dans le navigateur, du journal et de la configuration.
; La desinstallation defait le tout, et garde la base et les
; sauvegardes : un commerçant qui desinstalle ne veut pas perdre ses
; ventes.
;
; Construit par outils\construire_installeur_serveur.ps1, qui passe
; VERSION, SOURCE (le dossier des fichiers) et SORTIE.

Unicode True
!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"
!include "WordFunc.nsh"

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
; La recherche du serveur par les caisses (UDP 40000, noyau::decouverte) :
; le nom est celui que lit le serveur (reseau_local::REGLE_UDP).
!define REGLE_RECHERCHE "Gescom serveur (recherche)"
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
; D54 : la base d'un poste qui travaillait seul, et ce qu'elle contient.
Var BasePoste
Var Resume

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
Page custom PageConfiguration PageConfigurationValider
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "Le service « ${NOM} » tourne et demarre avec la machine.$\r$\n$\r$\nSa fenetre : menu Demarrer, Gescom, « Gescom Serveur » (port, adresse a taper sur les caisses, postes connectes).$\r$\n$\r$\nJournal : $DossierConfig\serveur.log$\r$\nConfiguration : $DossierConfig\serveur.json$\r$\n$\r$\nL'arreter / le relancer : sc stop ${SERVICE} / sc start ${SERVICE}"
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
  StrCpy $PortTexte "40000"
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
  ${NSD_CreateLabel} 0 50u 100% 12u "Port d'ecoute (40000 par defaut ; de 40000 a 40100, les seuls que les caisses joignent) :"
  Pop $0
  ${NSD_CreateNumber} 0 64u 60u 12u "$PortTexte"
  Pop $ChampPort
  ; Les droits par defaut de ProgramData laissent lire ce fichier a tous
  ; les comptes de la machine (BUILTIN\Utilisateurs : lecture, verifie le
  ; 01/11/2026) : le dire, plutot que promettre « administrateurs
  ; seulement ».
  ${NSD_CreateLabel} 0 86u 100% 36u "Le mot de passe PostgreSQL restera dans $DossierConfig\serveur.json, sur cette machine ; il n'est envoyé nulle part. Ce fichier se lit par tous les comptes de cette machine."
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
    StrCpy $PortTexte "40000"
  ${EndIf}
  ; Les caisses livrees ne joignent le serveur que de 40000 a 40100
  ; (leur CSP, tauri.conf.json, D39) : hors de la plage, aucune caisse
  ; ne le joindrait, sans un mot.
  ${If} $PortTexte < 40000
  ${OrIf} $PortTexte > 40100
    MessageBox MB_ICONEXCLAMATION|MB_OK "Les caisses Gescom ne joignent le serveur que sur un port de 40000 a 40100. Garder 40000, ou un port de cette plage."
    Abort
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------
;  Installation
; ---------------------------------------------------------------------

Section "Serveur" SEC_SERVEUR
  SectionIn RO
  SetOutPath "$INSTDIR"

  ; 0. Gescom (la caisse) ferme : ouverte, elle relancerait son serveur a
  ;    cote d'elle des qu'on l'arrete, sur la base du poste et le port
  ;    40000 — que le service doit prendre. Rien n'est encore change ici.
  Call ExigerGescomFerme

  ; Mise a jour : l'ancien service tient l'executable ; on le retire
  ; d'abord, sinon la copie echoue. Sans service en place, ces deux
  ; commandes echouent sans consequence.
  ${If} ${FileExists} "$INSTDIR\gescom-serveur.exe"
    nsExec::ExecToLog 'sc.exe stop ${SERVICE}'
    Sleep 2000
    nsExec::ExecToLog '"$INSTDIR\gescom-serveur.exe" --desinstaller-service'
    Sleep 1000
  ${EndIf}
  ; La fenetre ouverte tient son executable : on la ferme (elle ne garde
  ; rien, la console est servie par le service).
  ${If} ${FileExists} "$INSTDIR\gescom-serveur-fenetre.exe"
    nsExec::ExecToLog 'taskkill.exe /IM gescom-serveur-fenetre.exe /F'
  ${EndIf}
  ; Le serveur que la caisse lance a cote d'elle (poste seul, raccourci de
  ; demarrage) survit a sa fermeture : il tient la base du poste et le
  ; port 40000. Laisse en marche, le service ne pouvait pas ecouter, et la
  ; copie de la base du poste (4b) aurait manque ses dernieres ventes. Le
  ; service, lui, est deja arrete (mise a jour) ou pas encore installe.
  nsExec::ExecToLog 'taskkill.exe /IM gescom-serveur.exe /F'
  Sleep 1000

  File "${SOURCE}\gescom-serveur.exe"
  ; La fenetre du serveur (D25) : une coque qui affiche sa console.
  ; Elle a besoin de WebView2, present sur Windows 11 et sur les Windows
  ; 10 a jour ; il n'est pas embarque ici.
  File "${SOURCE}\gescom-serveur-fenetre.exe"
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

  ; 4. Le pare-feu : sur la plage du serveur (D39), profils Domaine et
  ;    Prive (pas Public : le reseau d'un hotel n'a pas a voir la base
  ;    d'un commerce). Toute la plage, comme outils/parefeu.ps1 : un
  ;    port change plus tard dans serveur.json reste joignable.
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${REGLE_PAREFEU}"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${REGLE_PAREFEU}" dir=in action=allow protocol=TCP localport=40000-40100 profile=domain,private'
  ;    Et la recherche du serveur par les caisses : UDP 40000, toujours
  ;    (meme quand le serveur sert sur un autre port de la plage), memes
  ;    profils.
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${REGLE_RECHERCHE}"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${REGLE_RECHERCHE}" dir=in action=allow protocol=UDP localport=40000 profile=domain,private'

  ; 4b. Un poste qui travaillait seul (D54) : sa base, si celle du service
  ;     est neuve. Avant le service : il n'a pas encore ouvert sa base.
  Call ReprendreBasePoste

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
  ; 6. La fenetre du serveur : elle lit le port dans serveur.json a chaque
  ;    ouverture, elle suit donc un port change depuis la console.
  CreateShortcut "$SMPROGRAMS\Gescom\Gescom Serveur.lnk" "$INSTDIR\gescom-serveur-fenetre.exe" "" "$INSTDIR\gescom-serveur-fenetre.exe" 0
  CreateShortcut "$SMPROGRAMS\Gescom\Journal du serveur.lnk" "notepad.exe" '"$DossierConfig\serveur.log"'
  CreateShortcut "$SMPROGRAMS\Gescom\Configuration du serveur.lnk" "notepad.exe" '"$DossierConfig\serveur.json"'
  ; Le port de l'installation : apres un changement de port depuis la
  ; console, c'est la fenetre « Gescom Serveur » qui suit, pas ce lien.
  CreateShortcut "$SMPROGRAMS\Gescom\Console du serveur.lnk" "http://localhost:$PortTexte"
SectionEnd

; ---------------------------------------------------------------------
;  D54 : Gescom ferme, et la base d'un poste qui travaillait seul
; ---------------------------------------------------------------------

; La caisse s'appelle gescom.exe (le nom du programme Rust, comme le
; modele de Tauri). tasklist voit les processus de toutes les sessions.
Function ExigerGescomFerme
  encore:
    nsExec::ExecToStack '"$SYSDIR\cmd.exe" /c tasklist /FI "IMAGENAME eq gescom.exe" /NH | "$SYSDIR\find.exe" /I "gescom.exe"'
    Pop $0
    Pop $1
    ${If} $0 == 0
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Gescom est ouvert sur ce poste. Le fermer (finir d'abord la vente en cours), puis cliquer Réessayer.$\r$\n$\r$\nAnnuler arrête l'installation : rien n'aura été changé." /SD IDCANCEL IDRETRY encore
      Quit
    ${EndIf}
FunctionEnd

; La ligne que rend `gescom-serveur --reprendre … --installeur` (dans $1) :
; « 1 234 ventes|210 articles|56 clients|31/10/2026 », en ASCII. Les
; textes, eux, sont ici : la sortie que nsExec capture ne garantit pas les
; accents. Rend le resume dans $Resume.
Function LireResume
  Push $2
  Push $3
  Push $4
  Push $5
  ${WordFind} "$1" "|" "+1" $2
  ${WordFind} "$1" "|" "+2" $3
  ${WordFind} "$1" "|" "+3" $4
  ${WordFind} "$1" "|" "+4" $5
  StrCpy $Resume "$2, $3, $4"
  ${If} $5 != "-"
  ${AndIf} $5 != ""
    StrCpy $Resume "$Resume ;$\r$\n   dernière vente le $5"
  ${EndIf}
  Pop $5
  Pop $4
  Pop $3
  Pop $2
FunctionEnd

; Le code de sortie d'une reprise refusee (dans $0) → la raison (dans $2).
; Les codes : noyau::reprise::codes.
Function RaisonDuRefus
  ${If} $0 == 11
    StrCpy $2 "la base de Gescom Serveur n'est plus neuve"
  ${ElseIf} $0 == 13
    StrCpy $2 "elle ne se lit pas (abîmée, ou pas une base Gescom)"
  ${ElseIf} $0 == 20
    StrCpy $2 "la copie ne correspondait pas à l'originale (Gescom était-il encore ouvert ?)"
  ${ElseIf} $0 == 21
    StrCpy $2 "la base de Gescom Serveur est ouverte par un autre programme"
  ${ElseIf} $0 == 22
    StrCpy $2 "l'écriture dans $DossierConfig a échoué (disque plein ?)"
  ${Else}
    StrCpy $2 "erreur inattendue (code $0)"
  ${EndIf}
FunctionEnd

; Un poste qui travaillait seul garde sa base dans le dossier de son
; utilisateur. L'installeur tourne en administrateur, sous le MEME compte
; quand c'est lui qui a lance l'installeur (le cas d'une boutique) : $APPDATA
; est alors le sien. Sous un autre compte (un technicien qui tape son mot
; de passe d'administrateur), $APPDATA est celui du technicien, rien n'est
; trouve : le guide (chapitre 9) donne la commande a la main.
;
; La commande fait le travail et le verifie (noyau/src/reprise.rs) ;
; l'installeur demande et affiche. Codes : noyau::reprise::codes.
Function ReprendreBasePoste
  StrCpy $BasePoste "$APPDATA\ml.gescom.app\gescom.db"
  ${IfNot} ${FileExists} "$BasePoste"
    Return
  ${EndIf}
  DetailPrint "Base de ce poste : $BasePoste"
  nsExec::ExecToStack '"$INSTDIR\gescom-serveur.exe" --reprendre "$BasePoste" --examiner --installeur'
  Pop $0
  Pop $1
  ${If} $0 == 0
    ; Base neuve, ou deja reprise (la marque reprise.json) : rien a dire.
    Return
  ${ElseIf} $0 == 10
    Call LireResume
    MessageBox MB_YESNO|MB_ICONQUESTION "Ce poste travaillait seul : il a déjà une base Gescom.$\r$\n   $BasePoste$\r$\n   $Resume.$\r$\n$\r$\nLa base de Gescom Serveur est neuve : elle ne contient rien.$\r$\n$\r$\nCopier la base de ce poste dans Gescom Serveur ?$\r$\n$\r$\nOui : la copie est vérifiée, puis le service démarre dessus.$\r$\nLa base du poste reste à sa place, sans changement.$\r$\n$\r$\nNon : Gescom Serveur démarre sur une base vide, et la caisse de ce poste s'y branchera. La base du poste reste à sa place ; pour la reprendre plus tard, relancer cet installeur." /SD IDYES IDYES copier
    DetailPrint "Base du poste : non copiée (refusé)."
    MessageBox MB_OK|MB_ICONINFORMATION "La base du poste n'est pas copiée : Gescom Serveur démarre sur une base vide.$\r$\n$\r$\nLa base du poste reste à sa place :$\r$\n   $BasePoste$\r$\nPour la reprendre plus tard, relancer cet installeur, tant que la base de Gescom Serveur n'a rien enregistré." /SD IDOK
    Return
    copier:
    DetailPrint "Copie de la base du poste dans Gescom Serveur…"
    nsExec::ExecToStack '"$INSTDIR\gescom-serveur.exe" --reprendre "$BasePoste" --installeur'
    Pop $0
    Pop $1
    ${If} $0 == 0
      Call LireResume
      DetailPrint "Base du poste copiée et vérifiée."
      MessageBox MB_OK|MB_ICONINFORMATION "La base du poste est copiée dans Gescom Serveur et vérifiée :$\r$\n   $Resume.$\r$\n$\r$\nLa base du poste reste à sa place ; Gescom ne s'en sert plus." /SD IDOK
    ${Else}
      Call RaisonDuRefus
      DetailPrint "Base du poste non copiée (code $0) : $2."
      MessageBox MB_OK|MB_ICONEXCLAMATION "La base du poste n'a pas été copiée : $2.$\r$\n$\r$\nRien n'est perdu : la base du poste est intacte. Gescom Serveur démarre sur sa base neuve. Pour reprendre la copie : guide d'installation, chapitre 9.$\r$\n$\r$\nLe détail : $DossierConfig\serveur.log" /SD IDOK
    ${EndIf}
  ${ElseIf} $0 == 11
    Call LireResume
    MessageBox MB_OK|MB_ICONEXCLAMATION "Ce poste a déjà une base Gescom avec des ventes :$\r$\n   $BasePoste$\r$\n   $Resume.$\r$\n$\r$\nLa base de Gescom Serveur a aussi des ventes. L'installeur ne choisit pas entre les deux : aucune n'est touchée, et Gescom Serveur garde la sienne.$\r$\n$\r$\nPour trancher, avec un technicien : guide d'installation, chapitre 9." /SD IDOK
  ${ElseIf} $0 == 12
    Call LireResume
    MessageBox MB_OK|MB_ICONEXCLAMATION "Ce poste a déjà une base Gescom avec des ventes :$\r$\n   $BasePoste$\r$\n   $Resume.$\r$\n$\r$\nGescom Serveur est réglé sur PostgreSQL : l'installeur ne sait pas y copier une base de poste. Rien n'est copié, la base du poste reste à sa place. Guide d'installation, chapitres 6 et 9." /SD IDOK
  ${ElseIf} $0 == 13
    MessageBox MB_OK|MB_ICONEXCLAMATION "Ce poste a un fichier de base Gescom qui ne se lit pas (abîmé, ou pas une base Gescom) :$\r$\n   $BasePoste$\r$\nIl n'est pas copié, et il reste à sa place. Gescom Serveur démarre sur sa base. Guide d'installation, chapitre 9." /SD IDOK
  ${Else}
    ; 14 (la base du serveur ne se lit pas), ou la commande n'a pas pu
    ; etre lancee : on ne touche a rien, on le note.
    DetailPrint "Base du poste : examen impossible (code $0), rien n'est copié."
    MessageBox MB_OK|MB_ICONEXCLAMATION "La base de ce poste n'a pas pu être comparée à celle de Gescom Serveur (code $0) : rien n'est copié, aucune base n'est touchée. Guide d'installation, chapitre 9." /SD IDOK
  ${EndIf}
FunctionEnd

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
  nsExec::ExecToLog 'taskkill.exe /IM gescom-serveur-fenetre.exe /F'
  nsExec::ExecToLog 'sc.exe stop ${SERVICE}'
  Sleep 2000
  nsExec::ExecToLog '"$INSTDIR\gescom-serveur.exe" --desinstaller-service'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${REGLE_PAREFEU}"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${REGLE_RECHERCHE}"'
  Delete "$INSTDIR\gescom-serveur.exe"
  Delete "$INSTDIR\gescom-serveur-fenetre.exe"
  Delete "$INSTDIR\gescom.cer"
  Delete "$INSTDIR\Desinstaller.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Gescom\Gescom Serveur.lnk"
  Delete "$SMPROGRAMS\Gescom\Journal du serveur.lnk"
  Delete "$SMPROGRAMS\Gescom\Configuration du serveur.lnk"
  Delete "$SMPROGRAMS\Gescom\Console du serveur.lnk"
  RMDir "$SMPROGRAMS\Gescom"
  DeleteRegKey HKLM "${CLE_DESINSTALL}"
  DeleteRegKey HKLM "Software\Gescom\Serveur"
  MessageBox MB_ICONINFORMATION|MB_OK "Le service est retire. La base, les sauvegardes et le journal sont conserves dans$\r$\n$DossierConfig"
SectionEnd
