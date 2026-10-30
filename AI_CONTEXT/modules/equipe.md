# Module : Gescom Équipe (personnel, paie, suivi client)

Le plan : [PLAN-EQUIPE.md](../PLAN-EQUIPE.md), D28 à D34. Une seconde
fenêtre qui ne parle qu'au serveur ; modules en version `Base` seule
(D29).

| Couche | Fichier | Rôle |
|---|---|---|
| Fenêtre | `equipe.html`, [src/equipe/](../../src/equipe/) | `AppEquipe` (connexion par `PageLogin`, session `gescom_equipe_session`, mot de passe à changer, écran « a besoin du serveur »), `modules.ts` (menu et droits), `Accueil`, `Bientot` |
| Coque | [src-tauri/equipe/](../../src-tauri/equipe/) | `gescom-equipe` : une commande, `lire_config_reseau` (serveur de la caisse du poste, sinon `127.0.0.1:40000` (ou le port de la plage retenu dans `poste.json`, D39) ; empreinte `…-equipe`) ; `npm run equipe:dev` / `equipe:build` |
| Personnel (pur) | [coeur/personnel.rs](../../src-tauri/noyau/src/coeur/personnel.rs) | `Remuneration` (au mois, à la journée, commission %, à la tâche — cumulables ; tout vide = rien de fixe), `dire` (« 60 000 F par mois + 2 % des ventes encaissées », D42), `valider` (montants bornés, `coeur::saisie::MONTANT_MAX`), `remuneration_a_ecrire(voit_la_paie, avant, saisie)` (28/10 : sans les droits de la paie, la rémunération et le plafond d'avance restent ceux de la fiche) |
| Personnel (base) | [personnel.rs](../../src-tauri/noyau/src/personnel.rs) | `lister_sur`, `lire_sur`, `creer_sur`, `modifier_sur` (= `modifier_selon_droits_sur(…, true)`), `modifier_selon_droits_sur(…, voit_la_paie)` (le serveur), `faire_partir_sur` (refuse une date avant des jours marqués après elle, en disant quoi faire ; `avertissements` : avance en cours, fiche validée à rectifier ; `avance_en_cours`), `faire_revenir_sur(id, date)` (après une interruption, la date de retour devient la date d'entrée — l'ancienne et le départ au journal — ; un retour au plus le lendemain du départ ne change rien ; refusé tant que la paie jusqu'au départ n'est sur aucune fiche validée, si la personne a un salaire, un tarif ou une commission) ; journal `employe_*` sans montant |
| Écran | [src/equipe/Personnel.tsx](../../src/equipe/Personnel.tsx) | onglets Fiches / Jours travaillés ; liste (départs à part), fiche (trois champs + « plus d'informations »), départ (avertissements affichés) / retour daté (« Confirmer le retour ») ; montants par `MoneyInput` ; sans la paie, la rémunération d'une fiche existante ne se coche plus |
| Jours (pur) | [coeur/presences.rs](../../src-tauri/noyau/src/coeur/presences.rs) | `valeur` (présent 1, demi 0,5, absent 0), `jours_travailles`, `compter` (présents, demis, absents — en entiers, pour la paie), `jours_du_mois`, `marquable` (ni demain, ni avant l'entrée, ni après le départ), `verifier_depart`, `refus_jour_fige` |
| Jours (base) | [presences.rs](../../src-tauri/noyau/src/presences.rs) | `lire_mois_sur` (grille ; les partis du mois y restent ; `fiches_validees` par personne), `jours_travailles_sur(employe, du, au)`, `marquer_sur` (ou effacer ; **refusé** sur un jour d'une fiche validée non en rectification — « … sur la fiche PAIE-… déjà validée : faire une rectificative … », jugé dans la transaction), `tous_presents_sur` (sans toucher une absence ni un jour figé : `figes`) |
| Écran jours | [src/equipe/JoursTravailles.tsx](../../src/equipe/JoursTravailles.tsx) | grille du mois, case qui tourne P → ½ → A → vide, numéro du jour = tous présents ; un jour payé par une fiche validée est grisé (« faire une rectificative ») |
| Avances (pur) | [coeur/avances.rs](../../src-tauri/noyau/src/coeur/avances.rs) | `reste(montant, retenu)`, `verifier(montant, en_cours, plafond, nom, voit_plafond)` (le plafond compte ce qui est en cours ; sans la paie, le refus ne le chiffre pas ; montant borné, addition vérifiée) |
| Avances (base) | [avances.rs](../../src-tauri/noyau/src/avances.rs) | `donner_avance_sur(base, employe, montant, Saisie { moyen, motif, date, mensualite, du_tiroir }, voit_la_paie)` (D42 : pris dans le tiroir → sortie de caisse `avance_salaire` dans la transaction, caisse ouverte exigée, datée d'aujourd'hui ; sinon hors caisse, datée de 31 jours en arrière au plus, jamais avant l'entrée ; rend `avertissement` si la paie de ce jour est validée) ; `donner_sur` / `donner_selon_droits_sur` (= hors caisse, aujourd'hui, tout retenu) ; `annuler_sur` (datée `annule_le`, seulement si rien n'est retenu ; prise dans le tiroir, l'argent y revient — `avance_salaire_annulee`, caisse ouverte exigée), `en_cours_sur`, `lister_sur` (`trop_verse_numero`, `mensualite`, `du_tiroir`) |
| Écran paie | [src/equipe/Paie.tsx](../../src/equipe/Paie.tsx) | onglets Avances / Fiches du mois / Cotisations ; donner (`MoneyInput`), en cours par personne, annuler ; une avance de trop-versé se dit « versé en trop sur PAIE-…, repris en avance » |
| Paie (pur) | [coeur/paie.rs](../../src-tauri/noyau/src/coeur/paie.rs) | **tout en entiers** (28/10) : `arrondi(num, den)` (au franc, la moitié s'éloignant de zéro), `millioniemes(pct)` ; `bornes_d_emploi`, `hors_emploi`, `nombre_de_jours`, `jours_du_mois_de`, `verifier_mois_entier` (au mois : un mois civil entier), `verifier_un_seul_mois`, `debut_du_compte` (un compte repris ne commissionne pas deux fois) ; `gains_calcules` (au mois : salaire × jours d'emploi / jours du mois, **toujours** ; au prorata, moins les absences et ½ par demi-journée — un jour non marqué compte comme présent ; jours × tarif en demi-journées ; % des ventes signées **moins les retours de la période**, jamais négatif), `ligne_saisie` (bornée), `retenir_avances`, `calculer` / `totaux` / `validable` (sommes vérifiées, `Result`), `Conditions` (ce qui a servi au calcul), `conditions_depuis_lignes` (une fiche d'avant le 28/10), `reste_a_verser`, `trop_verse_a_reprendre`, `MOYEN_TROP_VERSE` |
| Paie (base) | [paie.rs](../../src-tauri/noyau/src/paie.rs) | `preparer_sur` (refuse une période hors de l'emploi, et pour qui est payé au mois une période qui n'est pas un mois civil entier), `recalculer_sur`, `ajouter_ligne_sur`, `retirer_ligne_sur`, `supprimer_sur` (brouillon, vérifié au moment d'écrire), `valider_sur` (refuse ce qui n'est plus à jour ; n'écrit que si la fiche est encore un brouillon ; numéro `PAIE-AAAA-NNNNN`, avances retenues, conditions figées ; une rectificative crée l'avance du trop-versé), `rectifier_sur` (conditions figées de l'ancienne, ou relues sur ses lignes), `lire_sur`, `lister_sur(du, au)` (fiches + personnes employées au moins un jour de la période), `fiche_validee_du_jour_sur`, `fiches_validees_apres_sur`, `jour_paye_sur` |
| Payer (base) | [paie.rs](../../src-tauri/noyau/src/paie.rs) | `verser_avec_sur(base, fiche, montant, moyen, du_tiroir)` (D42 : pris dans le tiroir → sortie de caisse `salaire`, espèces, caisse ouverte exigée) ; `verser_sur` (= hors caisse ; pas plus que le reste), `salaires_par_mois_sur` (Rapports → CA mensuel : brut + charges des fiches validées, au mois de fin de période ; masqué sans la paie), `donnees_bulletin_sur` ; la fiche lue porte `verse`, `trop_verse` (repris en avance), `reste` (= net − versé + repris ; négatif : un trop-versé non repris), `versements` |
| Bulletin | [lib/genererBulletin.ts](../../src/lib/genererBulletin.ts), [ApercuBulletin.tsx](../../src/equipe/ApercuBulletin.tsx) | genre de document `bulletin` (Paramètres → Documents), aperçu = impression ; dit le trop-versé repris en avance |
| Cotisations | `coeur::paie::{Cotisation, valider_cotisation, cotisations, charges}`, `paie::{cotisations_sur, enregistrer_cotisation_sur, retirer_cotisation_sur}` (cotisation et historique dans **une** transaction), [Cotisations.tsx](../../src/equipe/Cotisations.tsx) | vides par défaut, seulement pour les déclarés ; la charge patronale hors du net |
| Écran fiches | [src/equipe/FichesPaie.tsx](../../src/equipe/FichesPaie.tsx) | le mois ; à préparer (au prorata ou non ; « entrée le », « partie le ») ; la liste (badge « versé en trop » / « payée » + « repris en avance ») ; la fiche : lignes, brut / retenues / net, report, trop-versé, ajouter / retirer une saisie (`MoneyInput`), recalculer, jeter, valider, rectifier |

**Table** `employe` (cloisonnée, RLS) : nom, fonction, modes de
rémunération, facultatifs (téléphone, entrée, pièce, contrat écrit +
date, déclaré + n° INPS, compte lié, magasin, note), `statut` actif /
partie. Un compte lié à une seule personne active par dossier (sinon
ses ventes compteraient deux fois dans les commissions).

**Droits** : `personnel:gerer` (écrire), lecture par qui gère ou paie
(`coeur::lecture` : `RefusSaufUne`) ; les montants
(`salaire_mensuel`, `tarif_journalier`, `commission_pct`,
`remuneration_dite`, `avance_max`) masqués sans `paie:preparer` / `paie:valider` ;
`modes` reste lisible. Depuis le 28/10 : les réponses de `creer_employe`,
`modifier_employe`, `faire_partir_employe`, `faire_revenir_employe` sont
masquées de même (et `avance_en_cours` sans le droit de lire les
avances) ; `modifier_employe` sans la paie **garde** la rémunération et
le plafond d'avance (`coeur::lecture::voit_la_paie`, lu par le serveur à
chaque appel) ; le journal PA se lit avec `paie:preparer` **ou**
`paie:valider` ; l'Historique masque les montants des gestes de paie
(fiche, versement, avance, cotisation) à qui ne voit pas la paie, et sa
recherche n'en fouille pas le détail (`historique::Filtre::sans_paie`).

**Table** `presence` (cloisonnée) : une ligne par personne et par jour
marqué (`UNIQUE (employe_id, jour)`) ; un jour sans ligne n'est pas su
(pour le salaire au mois au prorata, il compte comme présent — choix du
propriétaire, 28/10). Un jour couvert par une fiche validée est figé.

**Table** `avance` (cloisonnée, RLS) : montant, `retenu` (ce que les
fiches de paie en ont déjà pris), moyen, motif, statut `ouverte` /
`annulee`, `annule_le`. **Hors caisse** (D31/D32 révisées le 24/09) :
`mouvement_caisse_id` reste vide — sauf **pris dans le tiroir** (D42) :
il désigne la sortie de caisse écrite avec elle. `mensualite` (D42) : ce
qu'une paie en retient au plus, vide = tout ; `date_avance` peut être un
jour passé (`AAAA-MM-JJ`), `cree_le` garde l'instant de la saisie.
Droits : `personnel:avancer` pour donner et annuler ; lecture par qui
avance ou paie. `trop_verse_fiche_id` (28/10) : l'avance née d'un
trop-versé, créée dans la transaction qui valide la rectificative
(moyen `trop_verse`, journal `avance_trop_verse`, PA 421 / 422 — rien ne
sort de la trésorerie) ; elle suit la fiche qui vaut si celle-ci est
rectifiée de nouveau, et se retient sur la fiche suivante.

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
rectifier, verser : `paie:valider` ; lire : l'un des deux. `conditions`
(28/10, JSON) : ce qui a servi au calcul — rémunération, compte de la
commission, déclaration, cotisations — figé à la validation et repris
par la rectificative (D31 : septembre rectifié garde le salaire de
septembre) ; vide sur une fiche d'avant, relue alors sur ses lignes (le
journal `paie_rectificative` le dit). `commission_sur_encaisse` (D42,
dans `conditions`) : vrai pour toute fiche nouvelle — la commission
porte sur l'argent reçu dans la période sur les ventes du compte faites
pendant l'emploi, moins l'argent ou l'avoir rendu (retours non-échange
moins la dette éteinte, mode `retour`) ; absent = sur le vendu. Les
ventes d'avant la fin de la dernière fiche « sur le vendu »
(`vendu_jusqu_au`) ne comptent plus à leur règlement ; leurs retours se
déduisent pour leur valeur entière.

**Table** `versement_paie` (cloisonnée, RLS) : fiche, montant, moyen,
date (hors caisse : `mouvement_caisse_id` vide ; pris dans le tiroir,
D42 : la sortie de caisse). La
rectificative validée reprend les versements de l'ancienne.

Preuves : `personnel_base.rs` (6), `presences_base.rs` (5), `avances_base.rs` (5),
`paie_base.rs` (11), `paie_corrections_base.rs` (17 : la revue du 28/10,
un scénario par exemple), `paie_tiroir_base.rs` (8, D42 : tiroir, date,
étalement), `commission_encaisse_base.rs` (5, D42), trois moteurs ; route
`un_gestionnaire_sans_la_paie_n_efface_pas_le_salaire_et_un_montant_enorme_ne_tue_pas_la_base` ;
`src/lib/montant.test.ts` (`node --test`) ; bancs `f1-fenetre-equipe.mjs`,
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
`pour_utilisateur_id` (attribué à un compte actif **qui entre dans le
dossier**, C-2 : `acces_dossiers::role_dans_sur`, sinon refus — un
rappel attribué à qui ne peut pas ouvrir le dossier ne lui parviendrait
jamais, 22/10), `quand`, `quoi`, `fait` / `fait_le`. Se pose depuis la
fiche 360 ou l'onglet Prospects ; se marque fait des deux ; « Mes
rappels » les filtre sur la session.

**Pas de second fichier clients** (D34) : un **prospect** est une
ligne `client` avec `statut = 'prospect'` (colonnes `statut` et
`origine_prospect` — distincte d'`origine`, qui dit d'où vient la
*ligne* de données, app/demo/import). `creer_prospect_sur` appelle le
générateur de code du comptoir, `comptoir::prochain_code_client_sur`
(un seul format, 22/10). Il **redevient `client` tout seul à sa
première vente**, quel que soit son chemin : `argent::creer_vente_datee_sur`
et `valider_facture_sur` (fenêtre), `…_sur_base` (serveur) appellent
`prospect_devient_client[_sur]` dans la même transaction que la vente,
erreur propagée (ALERTES : pas de `let _` dans une transaction) — sans
bouton « convertir » à chercher.

**Droits** : `crm:suivre` (une seule permission, écriture et lecture ;
la lecture refuse sans elle, `coeur::lecture`), au catalogue depuis F-1.

Preuves : 2 unitaires `coeur::crm`, `crm_base.rs` (11, trois moteurs :
échange, fil fondu avec une relance, refus, rappel, rappel refusé hors
dossier, prospect qui devient client — au comptoir et par une facture
validée —, cloisonnement, détecteur) ; bancs
`h1-echanges-360.mjs`, `h2-rappels-prospects.mjs`.

## WhatsApp sans service payant (EQ-5 — D51)

| Couche | Fichier | Rôle |
|---|---|---|
| Règles (pur) | [src/lib/whatsapp.ts](../../src/lib/whatsapp.ts) | `lireNumero` (8 chiffres → +223 ; `+223`/`00223`/`223` ; `+`/`00` + autre indicatif, 8 à 15 chiffres ; sinon `{ ok: false, raison }`), `lienWhatsApp` (`https://wa.me/<chiffres>?text=…`, encodé), `raisonBulletin`, `texteBulletin`, `debutMessageClient`, `messageEcrit` |
| Écran | [src/equipe/WhatsApp.tsx](../../src/equipe/WhatsApp.tsx) | `ouvrirWhatsApp` (coque : plugin opener ; navigateur : `window.open`), `BoutonBulletinWhatsApp` (aperçu du bulletin), `MessageClientWhatsApp` (fiche 360) |
| Coque | [src-tauri/equipe](../../src-tauri/equipe) | `tauri-plugin-opener`, permission `opener:allow-open-url` limitée à `https://wa.me/*` |

**Rien ne part tout seul** : le lien ouvre WhatsApp sur la conversation,
le texte écrit, la personne appuie sur Envoyer. Un numéro douteux
désactive le bouton, la raison à côté ; la fiche n'est jamais réécrite.
**Bulletin** : fiche validée seulement ; période, numéro, net, déjà
versé, reste dû (« soldée » à zéro), l'avance reportée et le versé en
trop s'il y en a, la société ; le montant ne sort que de
`lire_donnees_bulletin`, refusée sans droit de paie ; rien n'est noté.
**Client** : la personne écrit après « Bonjour <nom>, » ; WhatsApp
ouvert, « Le message est-il parti ? » — Oui : `creer_echange` genre
`whatsapp`, le texte en `quoi` ; Non : rien, le texte reste. Aucune
commande serveur nouvelle.

Preuves : `src/lib/whatsapp.test.ts` (`node --test`) ; banc
`eq5-whatsapp.mjs` (le lien relu : numéro, texte ; numéro douteux ;
échange noté au Oui seulement).
