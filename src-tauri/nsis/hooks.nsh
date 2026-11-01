; Raccourcis du serveur Gescom, poses a l'installation.
;
; Sans eux, gescom-serveur.exe arrive bien dans le dossier
; d'installation mais rien ne le montre : le commercant ne sait pas
; qu'il existe, ne sait pas ou il est, et les caisses ne trouvent
; personne. Un fichier livre que personne n'ouvre n'est pas livre.
;
; installMode = currentUser : l'installeur n'a PAS les droits
; administrateur. On ne tente donc aucune regle de pare-feu ici — elle
; echouerait en silence, et laisser croire que le reseau est ouvert est
; pire que ne rien faire. Le serveur affiche lui-meme la commande a
; lancer au premier demarrage (reseau_local.rs).

; La desinstallation garde les donnees (D54) : la case « Supprimer les
; donnees de l'application » du modele de Tauri est retiree. Le detail
; est dans donnees_gardees.nsh.
!include "${__FILEDIR__}\donnees_gardees.nsh"

; A la place de la case, la page de confirmation dit ou restent les
; donnees, et que la desinstallation n'y touche pas. Pour repartir de
; zero, on efface ce dossier a la main (guide d'installation, chapitre 12).
!define MUI_UNCONFIRMPAGE_TEXT_TOP "Gescom va être retiré de cet ordinateur. Vos données n'y perdent rien : la base, les sauvegardes et le réglage du poste restent dans $APPDATA\ml.gescom.app."

!macro NSIS_HOOK_POSTINSTALL
  ; Un raccourci dans le menu Demarrer, a cote de l'application.
  CreateShortcut "$SMPROGRAMS\Gescom Serveur.lnk" "$INSTDIR\gescom-serveur.exe"

  ; Demarrage automatique de la session.
  ;
  ; Un serveur qu'il faut penser a lancer chaque matin est un serveur
  ; qui ne tournera pas : les caisses afficheront « Serveur
  ; injoignable » et personne ne fera le lien avec la fenetre noire
  ; qu'on a oublie d'ouvrir.
  ;
  ; Dans le demarrage de L'UTILISATEUR et non en service Windows : un
  ; service demanderait les droits administrateur que cet installeur
  ; n'a pas, et un enveloppeur de service en plus. Le poste principal
  ; d'une boutique reste allume et ouvert sur une session ; ce
  ; raccourci suffit.
  CreateShortcut "$SMSTARTUP\Gescom Serveur.lnk" "$INSTDIR\gescom-serveur.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; D54 : quoi qu'il arrive a la case, rien n'est efface.
  !insertmacro GESCOM_GARDER_LES_DONNEES

  ; Le serveur tient le fichier de base ouvert : tant qu'il tourne, la
  ; desinstallation ne peut pas remplacer ni supprimer les fichiers, et
  ; echoue avec un message que personne ne comprend.
  nsExec::Exec 'taskkill /IM gescom-serveur.exe /F'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$SMPROGRAMS\Gescom Serveur.lnk"
  Delete "$SMSTARTUP\Gescom Serveur.lnk"
!macroend
