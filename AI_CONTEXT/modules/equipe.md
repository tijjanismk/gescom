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
| Écran | [src/equipe/Personnel.tsx](../../src/equipe/Personnel.tsx) | onglets Fiches / Jours travaillés ; liste (départs à part), fiche (trois champs + « plus d'informations »), départ / retour |
| Jours (pur) | [coeur/presences.rs](../../src-tauri/noyau/src/coeur/presences.rs) | `valeur` (présent 1, demi 0,5, absent 0), `jours_travailles`, `jours_du_mois`, `marquable` (ni demain, ni avant l'entrée, ni après le départ) |
| Jours (base) | [presences.rs](../../src-tauri/noyau/src/presences.rs) | `lire_mois_sur` (grille ; les partis du mois y restent), `jours_travailles_sur(employe, du, au)` (lu par la paie), `marquer_sur` (ou effacer), `tous_presents_sur` (sans toucher une absence) |
| Écran jours | [src/equipe/JoursTravailles.tsx](../../src/equipe/JoursTravailles.tsx) | grille du mois, case qui tourne P → ½ → A → vide, numéro du jour = tous présents |
| Avances (pur) | [coeur/avances.rs](../../src-tauri/noyau/src/coeur/avances.rs) | `reste(montant, retenu)`, `verifier(montant, en_cours, plafond, nom)` (le plafond compte ce qui est en cours) |
| Avances (base) | [avances.rs](../../src-tauri/noyau/src/avances.rs) | `donner_sur` (sortie de caisse + avance + journal, une transaction, caisse ouverte exigée), `annuler_sur` (l'argent revient, seulement si rien n'est retenu), `en_cours_sur`, `lister_sur` |
| Écran paie | [src/equipe/Paie.tsx](../../src/equipe/Paie.tsx) | onglets Avances / Fiches du mois ; donner, en cours par personne, annuler |

**Table** `employe` (cloisonnée, RLS) : nom, fonction, modes de
rémunération, facultatifs (téléphone, entrée, pièce, contrat écrit +
date, déclaré + n° INPS, compte lié, magasin, note), `statut` actif /
partie. Un compte lié à une seule personne active par dossier (sinon
ses ventes compteraient deux fois dans les commissions).

**Droits** : `personnel:gerer` (écrire), lecture par qui gère ou paie
(`coeur::lecture` : `RefusSaufUne`) ; les montants
(`salaire_mensuel`, `tarif_journalier`, `commission_pct`,
`remuneration_dite`, `avance_max`) masqués sans `paie:preparer` / `paie:valider` ;
`modes` reste lisible.

**Table** `presence` (cloisonnée) : une ligne par personne et par jour
marqué (`UNIQUE (employe_id, jour)`) ; un jour sans ligne n'est pas su.

**Table** `avance` (cloisonnée, RLS) : montant, `retenu` (ce que les
fiches de paie en ont déjà pris), moyen, motif, statut `ouverte` /
`annulee`, `mouvement_caisse_id`. Le mouvement de caisse porte
`motif = 'avance'`, `categorie = 'salaire'`, `operation_id` = l'avance.
Droits : `personnel:avancer` pour donner et annuler ; lecture par qui
avance ou paie.

Preuves : `personnel_base.rs` (6), `presences_base.rs` (5), `avances_base.rs` (5),
trois moteurs ; bancs `f1-fenetre-equipe.mjs`, `f2-personnel.mjs`,
`f3-jours-travailles.mjs`, `g1-avances.mjs`.
