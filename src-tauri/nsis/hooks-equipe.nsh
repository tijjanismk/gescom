; Crochets NSIS de Gescom Equipe (src-tauri/equipe/tauri.conf.json).
;
; La desinstallation garde les donnees (D54). Equipe n'a pas de base :
; son dossier (%APPDATA%\ml.gescom.equipe) garde l'empreinte de ce poste,
; par laquelle le serveur le reconnait d'un jour a l'autre. La case du
; modele de Tauri l'effacait : le serveur aurait vu un poste nouveau, et
; l'ancien serait reste dans la liste. Meme retrait que pour la caisse.
!include "${__FILEDIR__}\donnees_gardees.nsh"

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro GESCOM_GARDER_LES_DONNEES
!macroend
