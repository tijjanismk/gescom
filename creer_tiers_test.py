#!/usr/bin/env python3
"""Crée 20 clients et 20 fournisseurs de test dans la base Gescom.

    python3 creer_tiers_test.py                  # aperçu, rien n'est écrit
    python3 creer_tiers_test.py --ecrire
    python3 creer_tiers_test.py --supprimer      # retire ce qu'il a créé

La base se trouve normalement dans :

    Windows : %APPDATA%\\ml.gescom.app\\gescom.db
    Linux   : ~/.local/share/ml.gescom.app/gescom.db

Sinon, passer le chemin : `--base "C:\\chemin\\gescom.db"`.

⚠ FERMER GESCOM AVANT DE LANCER. La base est en mode WAL : écrire
pendant que l'application tourne peut faire diverger ce qu'elle affiche
de ce qui est réellement en base, jusqu'au prochain redémarrage.

Tout ce qui est créé porte `origine = 'test'`, ce qui permet de tout
retirer d'un coup avec `--supprimer`. Le client générique (« Comptant »,
`est_generique = 1`) n'est jamais touché.
"""
import argparse
import os
import sqlite3
import sys
import uuid
from datetime import datetime

PRENOMS = [
    "Amadou", "Fatoumata", "Ibrahim", "Aminata", "Moussa", "Kadiatou",
    "Sekou", "Mariam", "Bakary", "Oumou", "Modibo", "Assitan",
    "Adama", "Djeneba", "Cheick", "Rokia", "Salif", "Nana",
    "Boubacar", "Hawa", "Drissa", "Sitan", "Yaya", "Bintou",
]
NOMS = [
    "Traoré", "Keïta", "Coulibaly", "Diarra", "Sidibé", "Touré",
    "Sangaré", "Konaté", "Doumbia", "Camara", "Cissé", "Fofana",
    "Dembélé", "Maïga", "Bagayoko", "Sissoko", "Kanté", "Samaké",
]
QUARTIERS = [
    "Badalabougou", "Faladié", "Hamdallaye", "Djelibougou", "Magnambougou",
    "Sébénikoro", "Kalaban Coura", "Lafiabougou", "Niaréla", "Sogoniko",
    "Banankabougou", "Yirimadio", "Missira", "Torokorobougou",
]
ENSEIGNES = [
    "Ets", "Sarl", "Comptoir", "Établissements", "Société", "Entreprise",
]
ACTIVITES = [
    "Matériaux", "Quincaillerie", "Import-Export", "Distribution",
    "Négoce", "Fournitures", "Bâtiment", "Électricité", "Plomberie",
]


def base_par_defaut():
    if os.name == "nt":
        return os.path.join(os.environ.get("APPDATA", ""),
                            "ml.gescom.app", "gescom.db")
    return os.path.expanduser("~/.local/share/ml.gescom.app/gescom.db")


def telephone(rng):
    # Numéros maliens : 8 chiffres, préfixes Orange (7x) et Moov (6x/9x).
    return f"{rng.choice(['70','71','76','77','65','66','90','91'])} " \
           f"{rng.randint(10,99)} {rng.randint(10,99)} {rng.randint(10,99)}"


def maintenant():
    return datetime.now().strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3]


def prochain_code(conn, prefixe, largeur=5):
    """MAX et non COUNT (D28) : une suppression ne rejoue pas un code."""
    cur = conn.execute(
        f"SELECT COALESCE(MAX(CAST(substr(code, -{largeur}) AS INTEGER)), 0) "
        f"FROM client WHERE code LIKE ?", (f"{prefixe}%",))
    return cur.fetchone()[0] + 1


def construire(rng, n):
    clients, fournisseurs = [], []
    vus = set()

    while len(clients) < n:
        nom = f"{rng.choice(PRENOMS)} {rng.choice(NOMS)}"
        if nom in vus:
            continue
        vus.add(nom)
        clients.append({
            "nom": nom,
            "telephone": telephone(rng),
            "adresse": f"{rng.choice(QUARTIERS)}, Bamako",
            # Un client sur trois a un NIF : ce sont les entreprises,
            # et c'est ce qui déclenche la mention sur la facture.
            "nif": f"{rng.randint(100000000, 999999999)}"
                   if rng.random() < 0.33 else None,
        })

    vus.clear()
    while len(fournisseurs) < n:
        nom = (f"{rng.choice(ENSEIGNES)} {rng.choice(NOMS)} "
               f"{rng.choice(ACTIVITES)}")
        if nom in vus:
            continue
        vus.add(nom)
        fournisseurs.append({
            "nom": nom,
            "telephone": telephone(rng),
            "adresse": f"{rng.choice(QUARTIERS)}, Bamako",
            "nif": f"{rng.randint(100000000, 999999999)}",
            # « Voisin » : le confrère chez qui on dépanne un article
            # manquant. Un sur quatre, c'est l'ordre de grandeur réel.
            "est_voisin": 1 if rng.random() < 0.25 else 0,
        })
    return clients, fournisseurs


def ecrire(conn, clients, fournisseurs):
    now = maintenant()
    n = prochain_code(conn, "CLI")

    for i, c in enumerate(clients):
        conn.execute(
            "INSERT INTO client (id, code, nom, telephone, adresse, nif,"
            " est_generique, actif, cree_le, modifie_le, cree_par,"
            " modifie_par, origine)"
            " VALUES (?,?,?,?,?,?,0,1,?,?,'test','test','test')",
            (str(uuid.uuid4()), f"CLI{n + i:05d}", c["nom"], c["telephone"],
             c["adresse"], c["nif"], now, now))

    for f in fournisseurs:
        conn.execute(
            "INSERT INTO fournisseur (id, nom, telephone, adresse, nif,"
            " est_voisin, cree_le, modifie_le)"
            " VALUES (?,?,?,?,?,?,?,?)",
            (str(uuid.uuid4()), f["nom"], f["telephone"], f["adresse"],
             f["nif"], f["est_voisin"], now, now))
    conn.commit()


def supprimer(conn):
    """Ne retire QUE ce que ce script a créé, jamais le générique."""
    c = conn.execute(
        "DELETE FROM client WHERE origine = 'test' AND est_generique = 0"
    ).rowcount
    # `fournisseur` n'a pas de colonne `origine` : on se rabat sur les
    # enseignes que ce script génère, et uniquement celles sans aucun
    # achat rattaché — supprimer un fournisseur qui a des factures
    # laisserait des dettes orphelines.
    f = conn.execute(
        "DELETE FROM fournisseur WHERE cree_le IS NOT NULL"
        "  AND (" + " OR ".join(f"nom LIKE '{e} %'" for e in ENSEIGNES) + ")"
        "  AND NOT EXISTS (SELECT 1 FROM piece_commerciale"
        "                  WHERE tiers_type='fournisseur' AND tiers_id = fournisseur.id)"
        "  AND NOT EXISTS (SELECT 1 FROM paiement_fournisseur"
        "                  WHERE fournisseur_id = fournisseur.id)"
    ).rowcount
    conn.commit()
    return c, f


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--base", default=None)
    p.add_argument("--nombre", type=int, default=20)
    p.add_argument("--ecrire", action="store_true")
    p.add_argument("--supprimer", action="store_true")
    p.add_argument("--graine", type=int, default=2026)
    a = p.parse_args()

    chemin = a.base or base_par_defaut()
    if not os.path.exists(chemin):
        print(f"Base introuvable : {chemin}", file=sys.stderr)
        print("Lancer Gescom une fois pour la créer, ou passer --base.",
              file=sys.stderr)
        return 1

    conn = sqlite3.connect(chemin)
    conn.execute("PRAGMA foreign_keys=ON")

    if a.supprimer:
        c, f = supprimer(conn)
        print(f"{c} client(s) et {f} fournisseur(s) de test supprimés.")
        return 0

    import random
    rng = random.Random(a.graine)
    clients, fournisseurs = construire(rng, a.nombre)

    if not a.ecrire:
        print(f"APERÇU — rien n'est écrit. Ajouter --ecrire pour appliquer.\n")
        print("Clients :")
        for c in clients[:5]:
            print(f"  {c['nom']:<24} {c['telephone']}  {c['adresse']}")
        print(f"  … et {len(clients) - 5} autres\n")
        print("Fournisseurs :")
        for f in fournisseurs[:5]:
            voisin = " [voisin]" if f["est_voisin"] else ""
            print(f"  {f['nom']:<38} {f['telephone']}{voisin}")
        print(f"  … et {len(fournisseurs) - 5} autres")
        return 0

    ecrire(conn, clients, fournisseurs)
    print(f"{len(clients)} clients et {len(fournisseurs)} fournisseurs créés.")
    print("Pour tout retirer : --supprimer")
    return 0


if __name__ == "__main__":
    sys.exit(main())
