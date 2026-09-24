# Module : Gescom Équipe (personnel, paie, suivi client)

Le plan : [PLAN-EQUIPE.md](../PLAN-EQUIPE.md), D28 à D34. Une seconde
fenêtre qui ne parle qu'au serveur ; modules en version `Base` seule
(D29).

| Couche | Fichier | Rôle |
|---|---|---|
| Fenêtre | `equipe.html`, [src/equipe/](../../src/equipe/) | `AppEquipe` (connexion par `PageLogin`, session `gescom_equipe_session`, mot de passe à changer, écran « a besoin du serveur »), `modules.ts` (menu et droits), `Accueil`, `Bientot` |
| Coque | [src-tauri/equipe/](../../src-tauri/equipe/) | `gescom-equipe` : une commande, `lire_config_reseau` (serveur de la caisse du poste, sinon `127.0.0.1:7300` ; empreinte `…-equipe`) ; `npm run equipe:dev` / `equipe:build` |
| Personnel (pur) | [coeur/personnel.rs](../../src-tauri/noyau/src/coeur/personnel.rs) | `Remuneration` (au mois, à la journée, commission %, à la tâche — cumulables ; tout vide = rien de fixe), `dire` (« 60 000 F par mois + 2 % des ventes »), `valider` |
| Personnel (base) | [personnel.rs](../../src-tauri/noyau/src/personnel.rs) | `lister_sur`, `lire_sur`, `creer_sur`, `modifier_sur`, `faire_partir_sur`, `faire_revenir_sur` ; journal `employe_*` sans montant |
| Écran | [src/equipe/Personnel.tsx](../../src/equipe/Personnel.tsx) | liste (départs à part), fiche (trois champs + « plus d'informations »), départ / retour |

**Table** `employe` (cloisonnée, RLS) : nom, fonction, modes de
rémunération, facultatifs (téléphone, entrée, pièce, contrat écrit +
date, déclaré + n° INPS, compte lié, magasin, note), `statut` actif /
partie. Un compte lié à une seule personne active par dossier (sinon
ses ventes compteraient deux fois dans les commissions).

**Droits** : `personnel:gerer` (écrire), lecture par qui gère ou paie
(`coeur::lecture` : `RefusSaufUne`) ; les montants
(`salaire_mensuel`, `tarif_journalier`, `commission_pct`,
`remuneration_dite`) masqués sans `paie:preparer` / `paie:valider` ;
`modes` reste lisible.

Preuves : `personnel_base.rs` (6, trois moteurs), bancs
`f1-fenetre-equipe.mjs`, `f2-personnel.mjs`.
