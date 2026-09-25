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
| Avances (base) | [avances.rs](../../src-tauri/noyau/src/avances.rs) | `donner_sur` (avance + journal, une transaction, **hors caisse**), `annuler_sur` (datée `annule_le`, seulement si rien n'est retenu), `en_cours_sur`, `lister_sur` |
| Écran paie | [src/equipe/Paie.tsx](../../src/equipe/Paie.tsx) | onglets Avances / Fiches du mois ; donner, en cours par personne, annuler |
| Paie (pur) | [coeur/paie.rs](../../src-tauri/noyau/src/coeur/paie.rs) | `gains_calcules` (mois, prorata jours travaillés / jours marqués, jours × tarif, % des ventes signées), `ligne_saisie` (tâche, prime, retenue), `retenir_avances` (les plus anciennes d'abord, le reste reporté), `calculer`, `totaux`, `validable`, `verifier_periode`, `se_chevauchent` |
| Paie (base) | [paie.rs](../../src-tauri/noyau/src/paie.rs) | `preparer_sur`, `recalculer_sur`, `ajouter_ligne_sur`, `retirer_ligne_sur`, `supprimer_sur` (brouillon), `valider_sur` (refuse ce qui n'est plus à jour ; numéro `PAIE-AAAA-NNNNN`, avances retenues), `rectifier_sur`, `lire_sur`, `lister_sur(du, au)` (fiches + personnes à préparer) |
| Payer (base) | [paie.rs](../../src-tauri/noyau/src/paie.rs) | `verser_sur` (hors caisse, pas plus que le reste), `salaires_par_mois_sur` (Rapports → CA mensuel : brut + charges des fiches validées, au mois de fin de période ; masqué sans la paie), `donnees_bulletin_sur` ; la fiche lue porte `verse`, `reste`, `versements` |
| Bulletin | [lib/genererBulletin.ts](../../src/lib/genererBulletin.ts), [ApercuBulletin.tsx](../../src/equipe/ApercuBulletin.tsx) | genre de document `bulletin` (Paramètres → Documents), aperçu = impression |
| Cotisations | `coeur::paie::{Cotisation, valider_cotisation, cotisations, charges}`, `paie::{cotisations_sur, enregistrer_cotisation_sur, retirer_cotisation_sur}`, [Cotisations.tsx](../../src/equipe/Cotisations.tsx) | vides par défaut, seulement pour les déclarés ; la charge patronale hors du net |
| Écran fiches | [src/equipe/FichesPaie.tsx](../../src/equipe/FichesPaie.tsx) | le mois ; à préparer (au prorata ou non) ; la liste ; la fiche : lignes, brut / retenues / net, report, ajouter / retirer une saisie, recalculer, jeter, valider, rectifier |

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
`annulee`, `annule_le`. **Hors caisse** (D31/D32 révisées le 24/09) :
`mouvement_caisse_id` reste vide.
Droits : `personnel:avancer` pour donner et annuler ; lecture par qui
avance ou paie.

**Tables** `fiche_paie` et `ligne_paie` (cloisonnées, RLS) : la fiche
se stocke (D31) ; `nom`, `fonction` figés à la paie ; statut
`brouillon` → `validee` → `remplacee` (par une rectificative, qui porte
`rectifie_id`) ; `brut`, `retenues` (en positif, avances comprises),
`net`, `reporte`. Une ligne : `genre` (base, jours, commission, tache,
prime, retenue, avance), `montant` signé, `saisie`, `source` (l'avance
retenue). Une fiche vivante par personne et par période (pas de
chevauchement). À la validation, `avance.retenu` monte (sans jamais
dépasser le montant) ; la rectificative rend d'abord ce que l'ancienne
avait retenu. Préparer / corriger : `paie:preparer` ; valider,
rectifier, verser : `paie:valider` ; lire : l'un des deux.

**Table** `versement_paie` (cloisonnée, RLS) : fiche, montant, moyen,
date (hors caisse : `mouvement_caisse_id` vide). La
rectificative validée reprend les versements de l'ancienne.

Preuves : `personnel_base.rs` (6), `presences_base.rs` (5), `avances_base.rs` (5),
`paie_base.rs` (10), trois moteurs ; bancs `f1-fenetre-equipe.mjs`,
`f2-personnel.mjs`, `f3-jours-travailles.mjs`, `g1-avances.mjs`,
`g2-fiches-paie.mjs`, `g3-verser-bulletin.mjs`, `g4-cotisations-journal.mjs`.

**Table** `cotisation` (cloisonnée, RLS) : libellé, `qui` (salarie /
employeur), `taux`, `plafond`, `compte` (43/44). Le journal PA :
[comptabilite.md](comptabilite.md).

## Suivi client (H-1/H-2 — D34)

| Couche | Fichier | Rôle |
|---|---|---|
| Suivi (pur) | [coeur/crm.rs](../../src-tauri/noyau/src/coeur/crm.rs) | `valider_echange` (genre connu, un texte), `valider_rappel`, `valider_prospect` |
| Suivi (base) | [crm.rs](../../src-tauri/noyau/src/crm.rs) | `creer_echange_sur`, `lister_echanges_sur` (fond les relances de créance dans le même fil), `creer_rappel_sur`, `marquer_rappel_fait_sur`, `lister_rappels_sur(pour, client, inclure_faits)`, `creer_prospect_sur`, `lister_prospects_sur` |
| Écran | [src/equipe/SuiviClients.tsx](../../src/equipe/SuiviClients.tsx) | onglets Clients (recherche + fiche 360) / Prospects / Mes rappels ; la fiche 360 : chiffres (`lire_fiche_client`), rappels, fil d'échanges |

**Table** `echange` (cloisonnée, RLS) : `client_id`, `genre` (appel,
visite, whatsapp, note), `quoi`, `suite_prevue`, `auteur_id`,
`date_echange`. Les relances de créance existantes (`relance_creance`,
[historique.md](comptabilite.md) — voir `relances.rs`) restent leur
table à elles ; D34 (« pas une liste de plus ») se tient à la
**lecture** : `crm::lister_echanges_sur` les joint (`relance_creance` →
`vente` → `client`) et les mêle au fil, triées par date.

**Table** `rappel` (cloisonnée, RLS) : `client_id`,
`pour_utilisateur_id` (attribué à un compte actif), `quand`, `quoi`,
`fait` / `fait_le`. Se pose depuis la fiche 360 ou l'onglet Prospects ;
se marque fait des deux ; « Mes rappels » les filtre sur la session.

**Pas de second fichier clients** (D34) : un **prospect** est une
ligne `client` avec `statut = 'prospect'` (colonnes `statut` et
`origine_prospect` — distincte d'`origine`, qui dit d'où vient la
*ligne* de données, app/demo/import). `creer_prospect_sur` réutilise le
générateur de code de `comptoir::creer_client_rapide_sur`. Il
**redevient `client` tout seul à sa première vente** :
`argent::creer_vente_datee_sur` (fenêtre) et `…_sur_base` (serveur)
basculent son statut dans la même transaction que la vente — sans
bouton « convertir » à chercher.

**Droits** : `crm:suivre` (une seule permission, écriture et lecture ;
la lecture refuse sans elle, `coeur::lecture`), au catalogue depuis F-1.

Preuves : 2 unitaires `coeur::crm`, `crm_base.rs` (9, trois moteurs :
échange, fil fondu avec une relance, refus, rappel, prospect qui
devient client, cloisonnement, détecteur) ; bancs
`h1-echanges-360.mjs`, `h2-rappels-prospects.mjs`.
