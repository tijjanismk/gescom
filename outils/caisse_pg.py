"""Une caisse, écran par écran, contre un serveur Gescom qui tourne.

Rejoue ce que la fenêtre fait : se connecter, puis ouvrir chaque écran
dans l'ordre d'une journée et cliquer là où l'écran clique — les mêmes
commandes, par le même pont HTTP (`/rpc`), avec les mêmes noms de
paramètres que `src/pages/*.tsx`.

C'est l'essai « à la main » qu'aucun scénario Rust ne remplace : les
scénarios disent ce que le SQL fait à la base, ceci dit ce que le
serveur rend à l'écran — un JSON, ou une erreur, pour chaque clic.

    python outils/caisse_pg.py http://127.0.0.1:7301 [admin] [admin123]

Le serveur doit tourner sur une base JETABLE : le script vend, achète,
rend, transfère, dépense. Jamais sur la base de la boutique.
"""

import json
import sys
import urllib.error
import urllib.request

RACINE = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:7301"
IDENTIFIANT = sys.argv[2] if len(sys.argv) > 2 else "admin"
MOT_DE_PASSE = sys.argv[3] if len(sys.argv) > 3 else "admin123"

jeton = None
resultats = []  # (ecran, commande, ok, message)


def http(chemin, corps, avec_jeton=True):
    entetes = {"Content-Type": "application/json"}
    if avec_jeton and jeton:
        entetes["Authorization"] = f"Bearer {jeton}"
    req = urllib.request.Request(
        RACINE + chemin, data=json.dumps(corps).encode(), headers=entetes, method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read().decode() or "null")
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read().decode())
        except Exception:
            return e.code, None


def rpc(ecran, commande, params=None, attendu_erreur=None):
    """Un clic. `attendu_erreur` : un refus est le comportement voulu."""
    statut, corps = http("/rpc", {"commande": commande, "params": params or {}})
    ok = statut == 200 and corps is not None and corps.get("etat") == "ok"
    if ok:
        resultats.append((ecran, commande, True, ""))
        return corps.get("donnee")
    message = (corps or {}).get("message", f"HTTP {statut}") if isinstance(corps, dict) else f"HTTP {statut}"
    if attendu_erreur and attendu_erreur in str(message):
        resultats.append((ecran, commande, True, f"refus attendu : {message[:70]}"))
        return None
    resultats.append((ecran, commande, False, message))
    return None


def connexion():
    global jeton
    statut, corps = http(
        "/connexion",
        {
            "identifiant": IDENTIFIANT,
            "mot_de_passe": MOT_DE_PASSE,
            "poste_nom": "caisse-essai",
            "poste_empreinte": "caisse-essai-pg",
            "version_protocole": 1,
        },
        avec_jeton=False,
    )
    if statut != 200:
        print(f"Connexion refusée ({statut}) : {corps}")
        sys.exit(1)
    jeton = corps["jeton"]
    print(f"connecté : {corps.get('nom')} ({corps.get('role')}), poste {corps.get('poste_id')}")
    return corps


def main():
    identite = connexion()

    # ---- Login : le mot de passe d'usine doit changer, puis on continue.
    if identite.get("doit_changer_mdp"):
        rpc("Login", "changer_mot_de_passe", {"ancienMdp": MOT_DE_PASSE, "nouveauMdp": "Essai-2026!"})
        rpc("Login", "changer_mot_de_passe", {"ancienMdp": "Essai-2026!", "nouveauMdp": MOT_DE_PASSE})

    # ---- Tableau de bord
    rpc("Dashboard", "lire_resume_dashboard", {"depotId": None})
    rpc("Dashboard", "lire_ventes_periode", {"periode": "semaine"})
    rpc("Dashboard", "lire_top_clients", {"limite": 5})
    rpc("Dashboard", "lire_top_articles", {"limite": 5})
    rpc("Dashboard", "lire_ventes_a_decouvert", {})

    # ---- Ventes (POS) : ce que l'écran charge, puis une vente
    articles = rpc("Ventes", "lire_articles_avec_unites", {"depotId": None}) or []
    clients = rpc("Ventes", "lire_clients", {}) or []
    generique = rpc("Ventes", "lire_client_generique", {})
    depots = rpc("Ventes", "lire_depots", {}) or []
    rpc("Ventes", "lire_stock_multi_depots", {})
    rpc("Ventes", "lire_config_scanner", {})
    depot = next((d for d in depots if d.get("est_defaut")), depots[0] if depots else {})
    sucre = next((a for a in articles if a["nom"] == "Sucre"), articles[0] if articles else None)
    riz = next((a for a in articles if a["nom"].startswith("Riz")), sucre)
    client_reel = next((c for c in clients if not c.get("est_generique")), None)
    if not (sucre and depot and generique and client_reel):
        print("La démo ne fournit pas de quoi vendre — arrêt.")
        return

    def ligne(article, quantite, prix=None):
        u = next(u for u in article["unites"] if u["facteur"] == 1.0)
        return {
            "article_id": article["id"], "unite_vente_id": u["id"],
            "depot_source_id": depot["id"], "source_approvisionnement": "stock",
            "quantite": quantite, "facteur": u["facteur"],
            "prix_reference": u["prix_reference"], "prix_pratique": prix or u["prix_reference"],
            "taux_tva": None, "a_decouvert": False,
        }

    prix_sucre = next(u for u in sucre["unites"] if u["facteur"] == 1.0)["prix_reference"]

    # Caisse fermée : la vente comptant doit être refusée, c'est la règle.
    rpc("Ventes", "creer_vente", {
        "clientId": generique["id"], "depotId": depot["id"], "modeReglement": "comptant",
        "lignes": [ligne(sucre, 1)], "montantPaye": prix_sucre, "modePaiement": "especes",
    }, attendu_erreur="CAISSE_FERMEE")

    # ---- Caisse : ouvrir
    rpc("Caisse", "lire_resume_caisse", {})
    sid = rpc("Caisse", "ouvrir_session_caisse", {"fondOuverture": 10000})
    rpc("Caisse", "lire_resume_caisse", {})

    # ---- Ventes : comptant au passage, crédit à un vrai client
    v1 = rpc("Ventes", "creer_vente", {
        "clientId": generique["id"], "depotId": depot["id"], "modeReglement": "comptant",
        "lignes": [ligne(sucre, 2), ligne(riz, 1)], "montantPaye": 2 * prix_sucre + next(u for u in riz["unites"] if u["facteur"] == 1.0)["prix_reference"],
        "modePaiement": "especes",
    })
    if v1:
        rpc("Ventes", "creer_facture_depuis_vente", {"venteId": v1["vente_id"], "clientId": generique["id"], "modeReglement": "comptant"})
    v2 = rpc("Ventes", "creer_vente", {
        "clientId": client_reel["id"], "depotId": depot["id"], "modeReglement": "credit",
        "lignes": [ligne(sucre, 3)], "montantPaye": 500, "modePaiement": "especes",
    })
    if v2:
        rpc("Ventes", "creer_facture_depuis_vente", {"venteId": v2["vente_id"], "clientId": client_reel["id"], "modeReglement": "credit"})
        rpc("Ventes", "enregistrer_cheque", {"venteId": v2["vente_id"], "numero": "000123", "banque": "BDM", "montant": 500})
    rpc("Ventes", "creer_client_rapide", {"nom": "Client essai PG", "telephone": "70 00 00 00"})
    rpc("Ventes", "creer_article_rapide", {"nom": "Article essai PG", "uniteBase": "pièce", "prixReference": 1000})

    # ---- Clients / Fiche client / Créances
    rpc("Clients", "lire_clients_pagines", {"page": 0, "limite": 20, "recherche": None, "avecCreancesSeulement": False, "ventesFiltre": None, "tri": None})
    rpc("Clients", "lire_creances_ouvertes", {"recherche": None})
    rpc("Clients", "lire_etat_creances_global", {})
    rpc("Clients", "lire_etat_creances_client", {"clientId": client_reel["id"]})
    rpc("FicheClient", "lire_fiche_client", {"clientId": client_reel["id"]})
    rpc("FicheClient", "lire_pieces_client", {"clientId": client_reel["id"], "typeFiltre": None})
    rpc("FicheClient", "lire_avoirs_client", {"clientId": client_reel["id"]})
    rpc("FicheClient", "lire_reglements_client", {"clientId": client_reel["id"]})
    if v2:
        rpc("FicheClient", "regler_creance", {"venteId": v2["vente_id"], "montant": 300, "mode": "especes"})
        rpc("FicheClient", "lire_donnees_recu", {"paiementId": "inconnu", "cote": "client"}, attendu_erreur="introuvable")
    rpc("FicheClient", "lire_config_signatures", {})
    rpc("FicheClient", "lire_parametres_societe", {})

    # ---- Pièces : devis → commande → BL + facture → validation → avoir
    devis = rpc("Pieces", "creer_piece", {
        "clientId": client_reel["id"], "typePiece": "devis",
        "lignes": [{"article_id": sucre["id"], "unite_vente_id": sucre["unites"][0]["id"], "quantite": 4, "prix_unitaire": prix_sucre, "remise_pct": 0, "taux_tva": 0}],
        "remiseGlobale": None, "dateEcheance": None, "note": "essai PG", "pieceOrigineId": None, "depotId": depot["id"],
    })
    rpc("Pieces", "lire_toutes_pieces_client", {"typeFiltre": None, "statut": None, "recherche": "dev", "dateDebut": None, "dateFin": None, "montantMin": None, "montantMax": None, "impayeSeulement": None, "enRetardSeulement": None, "clientId": None})
    if devis:
        rpc("Pieces", "lire_lignes_piece", {"pieceId": devis["id"]})
        rpc("Pieces", "lire_donnees_piece", {"pieceId": devis["id"]})
        copie = rpc("Pieces", "dupliquer_piece", {"pieceId": devis["id"]})
        if copie:
            rpc("Pieces", "modifier_piece", {"pieceId": copie["id"], "note": "copie modifiée", "dateEcheance": None, "remiseGlobale": 5, "lignes": None})
            rpc("Pieces", "annuler_piece", {"pieceId": copie["id"], "motif": "essai"})
        cmd = rpc("Pieces", "convertir_piece", {"pieceId": devis["id"], "nouveauType": "commande_client"})
        if cmd:
            deux = rpc("Pieces", "convertir_commande_en_livraison_et_facture", {"pieceId": cmd["id"]})
            if deux:
                fac = deux["facture"]["id"]
                rpc("Pieces", "lire_livraison_piece", {"pieceId": deux["bon_livraison"]["id"]})
                rpc("Pieces", "valider_facture", {"pieceId": fac, "modeReglement": "comptant", "modePaiement": "especes"})
                rpc("Pieces", "lire_vente_de_piece", {"pieceId": fac})
                rpc("Pieces", "annuler_facture_par_avoir", {"pieceId": fac, "modeRemboursement": "avoir", "moyen": None, "motif": "essai"})
    rpc("Pieces", "lire_config_bon_sortie", {})
    rpc("Pieces", "lire_config_suivi_livraison", {})

    # ---- Achats / Fournisseurs
    f = rpc("Achats", "creer_fournisseur", {"nom": "Grossiste essai PG", "telephone": None, "adresse": None, "nif": None, "email": None, "estVoisin": False})
    rpc("Achats", "lire_fournisseurs", {})
    if f:
        u = next(u for u in sucre["unites"] if u["facteur"] == 1.0)
        achat = rpc("Achats", "enregistrer_achat", {
            "fournisseurId": f["id"], "depotId": depot["id"],
            "lignes": [{"article_id": sucre["id"], "unite_vente_id": u["id"], "quantite": 10, "facteur": 1.0, "prix_achat": 500}],
            "modeReglement": "credit", "modePaiement": None, "acompte": 1000, "note": None, "pieceOrigineId": None,
        })
        rpc("Fournisseurs", "lire_fournisseurs_pagines", {"page": 0, "limite": 20, "recherche": "essai"})
        rpc("Fournisseurs", "lire_fournisseurs_avec_dettes", {})
        rpc("Fournisseurs", "lire_etat_dettes_global", {})
        rpc("FicheFournisseur", "lire_fournisseur_detail", {"fournisseurId": f["id"]})
        rpc("FicheFournisseur", "lire_etat_dette_fournisseur", {"fournisseurId": f["id"]})
        rpc("FicheFournisseur", "lire_fiche_fournisseur", {"fournisseurId": f["id"]})
        rpc("FicheFournisseur", "regler_dette_fournisseur", {"fournisseurId": f["id"], "montant": 1500, "mode": "especes", "note": "acompte"})
        rpc("Pieces", "lire_toutes_pieces_fournisseur", {"typeFiltre": None, "statut": None, "recherche": None, "fournisseurId": f["id"]})
        rpc("Achats", "lire_factures_fournisseur_retournables", {"fournisseurId": f["id"]})
        if achat:
            rpc("Achats", "enregistrer_retour_fournisseur", {
                "fournisseurId": f["id"], "depotId": None,
                "lignes": [{"article_id": sucre["id"], "unite_vente_id": u["id"], "quantite": 2, "facteur": 1.0, "prix_achat": 500}],
                "pieceOrigineId": achat["piece_id"], "modeResolution": "avoir", "modeEncaissement": None, "motif": "abîmé",
            })

    # ---- Retours client
    ventes = rpc("Retours", "lire_ventes_recentes", {}) or []
    rpc("Retours", "lire_avoirs_ouverts_tous", {})
    if v1 and ventes:
        vente = next((v for v in ventes if v["id"] == v1["vente_id"]), None)
        if vente:
            rpc("Retours", "enregistrer_retour", {
                "venteId": vente["id"], "ligneVenteId": vente["lignes"][0]["id"], "quantite": 1,
                "modeResolution": "remboursement", "modeEncaissement": "especes",
            })

    # ---- Stock / Magasins / Transferts
    rpc("Stock", "lire_stocks_pagines", {"page": 0, "limite": 20, "recherche": None, "aRegulariserSeulement": False, "categorieId": None})
    rpc("Stock", "lire_etat_stock", {"depotId": None, "avecZero": False})
    rpc("Stock", "lire_mouvements_stock", {"articleId": None, "depotId": None, "typeMouvement": None, "dateDebut": None, "dateFin": None, "limite": 50})
    rpc("Stock", "enregistrer_entree_stock", {"articleId": sucre["id"], "depotId": None, "quantite": 5, "prixAchat": 450, "fournisseurId": None})
    rpc("Stock", "enregistrer_retour_sans_facture", {"articleId": sucre["id"], "depotId": None, "quantite": 1, "fournisseurId": None, "motif": "rendu"})
    rpc("Stock", "enregistrer_ajustement_inventaire", {"articleId": sucre["id"], "depotId": depot["id"], "quantiteReelle": 150, "motif": "inventaire"})
    rpc("Magasins", "lire_depots_detail", {})
    annexe = rpc("Magasins", "creer_depot", {"nom": "Annexe essai", "estDefaut": False})
    if annexe:
        rpc("Magasins", "renommer_depot", {"depotId": annexe["id"], "nom": "Annexe essai 2"})
        rpc("Magasins", "lire_stock_depot", {"depotId": annexe["id"]})
        rpc("Magasins", "lire_stock_article_depots", {"articleId": sucre["id"]})
        rpc("Magasins", "lire_resume_par_depot", {"dateDebut": None, "dateFin": None})
        u = next(u for u in sucre["unites"] if u["facteur"] == 1.0)
        t = rpc("Transferts", "enregistrer_transfert", {
            "depotSource": depot["id"], "depotDest": annexe["id"],
            "lignes": [{"article_id": sucre["id"], "unite_vente_id": u["id"], "quantite": 5, "facteur": 1.0}], "motif": "réassort",
        })
        rpc("Transferts", "lire_transferts", {"limite": 20})
        if t:
            rpc("Transferts", "lire_bon_transfert", {"bon": t["bon"]})
        rpc("Magasins", "desactiver_depot", {"depotId": annexe["id"], "force": True})
        rpc("Magasins", "reactiver_depot", {"depotId": annexe["id"]})

    # ---- Caisse : dépense, journée, clôture, historique
    d = rpc("Caisse", "enregistrer_depense", {"montant": 500, "libelle": "Taxi", "categorie": "transport", "moyen": "especes"})
    if d:
        rpc("Caisse", "modifier_depense", {"mouvementId": d, "montant": 700, "libelle": None, "categorie": None})
    rpc("Caisse", "lire_depenses_du_jour", {})
    rpc("Caisse", "lire_mouvements_caisse_du_jour", {})
    rpc("Journal", "lire_journal_du_jour", {"date": None, "depotId": None})
    if sid:
        rpc("Caisse", "lire_mouvements_session", {"sessionId": sid})
        rpc("Caisse", "fermer_session_caisse", {"sessionId": sid, "especesComptees": 12000})
    rpc("Caisse", "lire_sessions_caisse", {"limite": 10})
    rpc("Caisse", "lire_rapport_ecarts", {"jours": 30})
    rpc("Caisse", "lire_mode_caisse", {})

    # ---- Rapports / Relances / Chèques / Chantiers
    for c, p in [
        ("lire_rapport_ca_mensuel", {"nbMois": 6}),
        ("lire_rapport_top_clients", {"dateDebut": "2026-01-01", "dateFin": "2026-12-31", "limite": 10}),
        ("lire_rapport_top_articles", {"dateDebut": "2026-01-01", "dateFin": "2026-12-31", "limite": 10}),
        ("lire_rapport_creances", {}),
        ("lire_rapport_stock", {}),
        ("lire_rapport_tva", {"dateDebut": "2026-01-01", "dateFin": "2026-12-31"}),
    ]:
        rpc("Rapports", c, p)
    rpc("Relances", "lire_creances_relances", {"enRetardSeulement": False})
    rpc("Relances", "lire_stats_relances", {})
    if v2:
        rpc("Relances", "enregistrer_relance", {"venteId": v2["vente_id"], "canal": "whatsapp", "note": "rappel"})
        rpc("Relances", "lire_historique_relances", {"venteId": v2["vente_id"]})
    rpc("Cheques", "lire_cheques", {"statut": None})
    rpc("Chantiers", "lire_taux_tva", {})
    rpc("Chantiers", "lire_resume_tva", {"dateDebut": "2026-01-01", "dateFin": "2026-12-31"})
    rpc("Chantiers", "lire_dettes_fournisseurs", {})
    rpc("Chantiers", "lire_irrecouvrable", {})
    rpc("Chantiers", "lire_config_avoirs", {})
    rpc("Chantiers", "lire_avoirs_expires", {})
    rpc("Chantiers", "expirer_avoirs", {})

    # ---- Paramètres
    rpc("Parametres", "lire_parametres_societe", {})
    rpc("Parametres", "lire_categories", {})
    rpc("Parametres", "lire_articles_complets", {})
    rpc("Parametres", "lire_roles", {})
    rpc("Parametres", "lire_catalogue_permissions", {})
    rpc("Parametres", "lire_utilisateurs", {})
    rpc("Parametres", "lire_config_sauvegarde", {})
    rpc("Parametres", "diagnostiquer_base", {})
    rpc("Parametres", "lire_articles_codes_barres", {"sansCodeSeulement": False})
    rpc("Parametres", "generer_code_barre", {"articleId": sucre["id"]})
    rpc("Parametres", "lire_stocks", {})
    rpc("Parametres", "exporter_articles_csv", {})
    rpc("Parametres", "lire_modeles", {"genre": None})
    rpc("Parametres", "lire_postes", {})
    rpc("Parametres", "lire_sessions_reseau", {})
    rpc("Parametres", "lire_logo_base64", {})
    rpc("Parametres", "sauvegarder_base", {"dossierDestination": "C:/Temp/gescom-essai-sauvegarde"})

    # ---- Bilan
    ko = [r for r in resultats if not r[2]]
    print(f"\n{len(resultats)} clics, {len(resultats) - len(ko)} ok, {len(ko)} en erreur")
    ecran = None
    for e, c, ok, m in resultats:
        if e != ecran:
            print(f"\n[{e}]")
            ecran = e
        print(f"  {'ok ' if ok else 'ERR'} {c}{('  — ' + m) if m else ''}")
    sys.exit(1 if ko else 0)


if __name__ == "__main__":
    main()
