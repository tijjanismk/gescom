; La desinstallation ne touche jamais aux donnees (D54, 01/11/2026).
;
; Le modele NSIS de Tauri (tauri-cli 2.11.4) ajoute a la page de
; confirmation de la desinstallation une case « Supprimer les donnees de
; l'application ». Cochee, elle fait, en fin de section :
;
;     RmDir /r "$APPDATA\${BUNDLEID}"
;     RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
;
; Pour la caisse, c'est %APPDATA%\ml.gescom.app : la base d'un poste
; seul, ses sauvegardes et poste.json. Le proprietaire : « LA RETIRER — la
; desinstallation ne touche jamais aux donnees ; pour repartir de zero, on
; efface le dossier a la main et le guide dit ou. »
;
; Deux verrous, sans recopier le modele de Tauri (900 lignes a suivre a
; chaque mise a jour de tauri-cli) :
;
; 1. La case est creee INVISIBLE. Le modele la cree dans un.ConfirmShow
;    avec le style ${__NSD_CheckBox_STYLE} de nsDialogs.nsh, que MUI2
;    inclut avant ce fichier ; c'est le seul endroit du modele qui cree
;    une case nsDialogs. On redefinit ce style sans WS_VISIBLE. (Si un
;    jour le symbole disparait, !undef echoue a la compilation : on le
;    saura.)
; 2. NSIS_HOOK_PREUNINSTALL, premiere ligne de la section Uninstall, remet
;    l'etat lu en quittant la page a 0 — avant la ligne RmDir, qui le lit
;    en fin de section. (Si la variable change de nom, StrCpy echoue a la
;    compilation : jamais un effacement en silence.)
;
; Inclus par hooks.nsh (la caisse) et hooks-equipe.nsh (Gescom Equipe).

!ifndef GESCOM_DONNEES_GARDEES
!define GESCOM_DONNEES_GARDEES

!undef __NSD_CheckBox_STYLE
!define __NSD_CheckBox_STYLE ${WS_CHILD}|${WS_CLIPSIBLINGS}|${BS_TEXT}|${BS_VCENTER}|${BS_AUTOCHECKBOX}|${BS_MULTILINE}

!macro GESCOM_GARDER_LES_DONNEES
  StrCpy $DeleteAppDataCheckboxState 0
!macroend

!endif
