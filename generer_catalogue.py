#!/usr/bin/env python3
"""Catalogue de test — 100 articles, avec conditionnements.

    python3 generer_catalogue.py > catalogue_test.csv

Respecte les conventions du projet :

  - `unite_base` = la plus petite unité vendable (D39). La première
    ligne d'un article porte donc TOUJOURS Facteur = 1.
  - `Facteur` s'exprime en unités de base, jamais emboîté : une palette
    de 6 cartons de 12 vaut 72, pas 6.
  - Codes-barres EAN-13 valides, préfixe interne 20 (D34) : ils ne
    peuvent entrer en collision avec aucun code du commerce.
  - Un code par CONDITIONNEMENT (D45) : le carton a le sien.
  - Prix en FCFA entiers (D10).

Les prix de pack incluent une remise de gros de 3 à 8 %, ce qui est
réaliste — et permet de vérifier que l'avertissement de cohérence des
prix (seuil 30 %) ne se déclenche pas à tort.
"""
import random
import sys

random.seed(2026)   # catalogue reproductible d'un lancement à l'autre

ENTETE = "Nom;Categorie;Unite;Facteur;Prix;Prix achat;TVA %;Code barre;Stock"


def cle_ean13(douze: str) -> int:
    """Clé de contrôle EAN-13 — même algorithme que coeur/codebarre.rs."""
    somme = sum(int(c) * (1 if i % 2 == 0 else 3)
                for i, c in enumerate(douze))
    return (10 - (somme % 10)) % 10


_sequence = 0


def ean13() -> str:
    global _sequence
    _sequence += 1
    douze = f"20{_sequence:010d}"
    return f"{douze}{cle_ean13(douze)}"


# (nom, catégorie, unité de base, prix unitaire, marge %, [(pack, facteur)])
CATALOGUE = [
    # --- Matériaux de construction -----------------------------------
    ("Ciment CPA 45", "Construction", "sac", 5500, 12, [("palette", 40)]),
    ("Ciment CPJ 35", "Construction", "sac", 4900, 12, [("palette", 40)]),
    ("Fer à béton 6mm", "Construction", "barre", 2200, 15, [("botte", 20)]),
    ("Fer à béton 8mm", "Construction", "barre", 3100, 15, [("botte", 20)]),
    ("Fer à béton 10mm", "Construction", "barre", 4200, 15, [("botte", 20)]),
    ("Fer à béton 12mm", "Construction", "barre", 5800, 15, [("botte", 20)]),
    ("Fil d'attache", "Construction", "kg", 1200, 20, [("rouleau", 25)]),
    ("Brique creuse 15", "Construction", "pièce", 350, 25, [("palette", 200)]),
    ("Brique pleine 20", "Construction", "pièce", 480, 25, [("palette", 150)]),
    ("Sable fin", "Construction", "m³", 18000, 30, []),
    ("Gravier 5/15", "Construction", "m³", 22000, 30, []),
    ("Tôle bac alu 3m", "Construction", "pièce", 8500, 18, [("paquet", 10)]),
    ("Tôle ondulée 2m", "Construction", "pièce", 6200, 18, [("paquet", 10)]),
    ("Chaux hydraulique", "Construction", "sac", 3800, 15, [("palette", 40)]),
    ("Enduit de façade", "Construction", "sac", 4200, 15, [("palette", 40)]),

    # --- Plomberie ---------------------------------------------------
    ("Tuyau PVC 63mm", "Plomberie", "mètre", 1800, 22, [("barre 6m", 6)]),
    ("Tuyau PVC 90mm", "Plomberie", "mètre", 2600, 22, [("barre 6m", 6)]),
    ("Tuyau PVC 110mm", "Plomberie", "mètre", 3400, 22, [("barre 6m", 6)]),
    ("Coude PVC 90°", "Plomberie", "pièce", 900, 30, [("carton", 25)]),
    ("Té PVC 63mm", "Plomberie", "pièce", 1200, 30, [("carton", 25)]),
    ("Colle PVC 250g", "Plomberie", "pot", 2800, 25, [("carton", 12)]),
    ("Robinet laiton 1/2", "Plomberie", "pièce", 3500, 28, [("carton", 20)]),
    ("Siphon lavabo", "Plomberie", "pièce", 2400, 28, [("carton", 12)]),
    ("Joint téflon", "Plomberie", "rouleau", 350, 40, [("boîte", 50)]),
    ("Réservoir 500L", "Plomberie", "pièce", 68000, 15, []),
    ("Réservoir 1000L", "Plomberie", "pièce", 115000, 15, []),
    ("Pompe immergée 1CV", "Plomberie", "pièce", 145000, 12, []),

    # --- Électricité -------------------------------------------------
    ("Câble 1.5mm²", "Électricité", "mètre", 320, 30, [("rouleau 100m", 100)]),
    ("Câble 2.5mm²", "Électricité", "mètre", 480, 30, [("rouleau 100m", 100)]),
    ("Câble 4mm²", "Électricité", "mètre", 750, 30, [("rouleau 100m", 100)]),
    ("Câble 6mm²", "Électricité", "mètre", 1100, 30, [("rouleau 100m", 100)]),
    ("Interrupteur simple", "Électricité", "pièce", 850, 35, [("carton", 50)]),
    ("Prise 2P+T", "Électricité", "pièce", 1100, 35, [("carton", 50)]),
    ("Disjoncteur 16A", "Électricité", "pièce", 3200, 25, [("carton", 12)]),
    ("Disjoncteur 32A", "Électricité", "pièce", 4500, 25, [("carton", 12)]),
    ("Tableau 12 modules", "Électricité", "pièce", 12000, 20, []),
    ("Ampoule LED 9W", "Électricité", "pièce", 900, 40, [("carton", 100)]),
    ("Ampoule LED 15W", "Électricité", "pièce", 1400, 40, [("carton", 100)]),
    ("Réglette LED 1.2m", "Électricité", "pièce", 4800, 28, [("carton", 20)]),
    ("Gaine ICTA 20mm", "Électricité", "mètre", 280, 35, [("couronne 50m", 50)]),
    ("Boîte de dérivation", "Électricité", "pièce", 700, 38, [("carton", 50)]),
    ("Panneau solaire 150W", "Électricité", "pièce", 78000, 15, []),
    ("Batterie solaire 100Ah", "Électricité", "pièce", 125000, 12, []),
    ("Régulateur 30A", "Électricité", "pièce", 22000, 18, []),

    # --- Peinture ----------------------------------------------------
    ("Peinture blanche mate", "Peinture", "litre", 2200, 25, [("bidon 20L", 20)]),
    ("Peinture blanche satinée", "Peinture", "litre", 2800, 25, [("bidon 20L", 20)]),
    ("Peinture bleue", "Peinture", "litre", 3100, 25, [("bidon 20L", 20)]),
    ("Peinture verte", "Peinture", "litre", 3100, 25, [("bidon 20L", 20)]),
    ("Peinture rouge", "Peinture", "litre", 3400, 25, [("bidon 20L", 20)]),
    ("Diluant synthétique", "Peinture", "litre", 1800, 28, [("bidon 5L", 5)]),
    ("Rouleau peinture 22cm", "Peinture", "pièce", 1500, 40, [("carton", 24)]),
    ("Pinceau 50mm", "Peinture", "pièce", 800, 45, [("carton", 24)]),
    ("Papier abrasif", "Peinture", "feuille", 250, 50, [("paquet", 50)]),
    ("Enduit de lissage", "Peinture", "sac", 5200, 20, [("palette", 40)]),

    # --- Quincaillerie -----------------------------------------------
    ("Pointe 40mm", "Quincaillerie", "kg", 1300, 25, [("carton", 25)]),
    ("Pointe 60mm", "Quincaillerie", "kg", 1300, 25, [("carton", 25)]),
    ("Pointe 80mm", "Quincaillerie", "kg", 1400, 25, [("carton", 25)]),
    ("Vis à bois 4x40", "Quincaillerie", "pièce", 25, 60, [("boîte 200", 200)]),
    ("Vis à bois 5x60", "Quincaillerie", "pièce", 40, 60, [("boîte 200", 200)]),
    ("Cheville 8mm", "Quincaillerie", "pièce", 30, 60, [("boîte 100", 100)]),
    ("Boulon M10", "Quincaillerie", "pièce", 180, 45, [("boîte 50", 50)]),
    ("Cadenas 50mm", "Quincaillerie", "pièce", 2500, 35, [("carton", 12)]),
    ("Serrure encastrée", "Quincaillerie", "pièce", 8500, 28, [("carton", 10)]),
    ("Charnière 100mm", "Quincaillerie", "paire", 1200, 40, [("carton", 25)]),
    ("Chaîne galvanisée 6mm", "Quincaillerie", "mètre", 900, 30, [("rouleau 30m", 30)]),
    ("Cadenas à code", "Quincaillerie", "pièce", 4200, 32, [("carton", 12)]),

    # --- Outillage ---------------------------------------------------
    ("Marteau 500g", "Outillage", "pièce", 3500, 35, [("carton", 12)]),
    ("Truelle 200mm", "Outillage", "pièce", 2200, 40, [("carton", 12)]),
    ("Niveau à bulle 60cm", "Outillage", "pièce", 4500, 35, [("carton", 10)]),
    ("Mètre ruban 5m", "Outillage", "pièce", 1800, 45, [("carton", 24)]),
    ("Scie égoïne", "Outillage", "pièce", 3800, 35, [("carton", 12)]),
    ("Pelle ronde", "Outillage", "pièce", 4200, 30, [("paquet", 10)]),
    ("Pioche", "Outillage", "pièce", 5500, 30, [("paquet", 10)]),
    ("Brouette 100L", "Outillage", "pièce", 32000, 20, []),
    ("Bétonnière 350L", "Outillage", "pièce", 850000, 12, []),
    ("Perceuse 750W", "Outillage", "pièce", 42000, 22, []),
    ("Meuleuse 125mm", "Outillage", "pièce", 38000, 22, []),
    ("Disque à tronçonner", "Outillage", "pièce", 1200, 45, [("carton", 25)]),
    ("Échelle alu 3m", "Outillage", "pièce", 55000, 18, []),
    ("Escabeau 5 marches", "Outillage", "pièce", 28000, 20, []),

    # --- Menuiserie --------------------------------------------------
    ("Planche coffrage 3m", "Menuiserie", "pièce", 4500, 20, [("paquet", 10)]),
    ("Chevron 7x7", "Menuiserie", "mètre", 1600, 22, [("botte", 12)]),
    ("Contreplaqué 5mm", "Menuiserie", "plaque", 12000, 18, [("paquet", 10)]),
    ("Contreplaqué 10mm", "Menuiserie", "plaque", 19000, 18, [("paquet", 10)]),
    ("Porte isoplane", "Menuiserie", "pièce", 35000, 20, []),
    ("Fenêtre alu 1m20", "Menuiserie", "pièce", 68000, 18, []),
    ("Colle à bois 1kg", "Menuiserie", "pot", 3200, 30, [("carton", 12)]),
    ("Vernis incolore", "Menuiserie", "litre", 4200, 25, [("bidon 5L", 5)]),

    # --- Divers ------------------------------------------------------
    ("Bâche 4x5m", "Divers", "pièce", 8500, 30, [("paquet", 10)]),
    ("Corde nylon 10mm", "Divers", "mètre", 450, 40, [("rouleau 50m", 50)]),
    ("Sac poubelle 100L", "Divers", "pièce", 150, 55, [("rouleau 20", 20)]),
    ("Gants de chantier", "Divers", "paire", 900, 50, [("carton", 60)]),
    ("Casque de chantier", "Divers", "pièce", 4500, 35, [("carton", 12)]),
    ("Masque anti-poussière", "Divers", "pièce", 350, 55, [("boîte 50", 50)]),
    ("Lunettes de protection", "Divers", "pièce", 2200, 45, [("carton", 24)]),
    ("Bottes de sécurité", "Divers", "paire", 12000, 30, []),
    ("Ruban adhésif large", "Divers", "rouleau", 800, 50, [("carton", 36)]),
    ("Bombe de peinture", "Divers", "pièce", 2400, 40, [("carton", 12)]),
    ("Extincteur 6kg", "Divers", "pièce", 28000, 25, []),
    ("Cadenas antivol vélo", "Divers", "pièce", 3800, 38, [("carton", 12)]),
]

TVA_PAR_CATEGORIE = {
    # Les matériaux de base sont souvent exonérés au Mali ; le reste à
    # 18 %. Le mélange sert justement à tester les deux chemins.
    "Construction": 0,
    "Menuiserie": 0,
}


def echapper(v: str) -> str:
    return f'"{v}"' if ";" in v or '"' in v else v


def main():
    lignes = [ENTETE]
    for nom, cat, unite, prix, marge, packs in CATALOGUE:
        prix_achat = round(prix * 100 / (100 + marge))
        tva = TVA_PAR_CATEGORIE.get(cat, 18)
        stock = random.choice([0, 12, 25, 40, 60, 120, 250, 500])

        # Unité de base — facteur 1, TOUJOURS en premier (D39).
        lignes.append(";".join([
            echapper(nom), cat, unite, "1", str(prix), str(prix_achat),
            str(tva), ean13(), str(stock),
        ]))

        # Conditionnements — remise de gros de 3 à 8 %.
        for libelle, facteur in packs:
            remise = random.uniform(0.03, 0.08)
            prix_pack = round(prix * facteur * (1 - remise))
            lignes.append(";".join([
                echapper(nom), cat, libelle, str(facteur), str(prix_pack),
                str(prix_achat), str(tva), ean13(), str(stock),
            ]))

    # BOM UTF-8 : sans lui Excel francophone affiche « Café ».
    sys.stdout.write("\ufeff" + "\n".join(lignes) + "\n")

    nb_articles = len(CATALOGUE)
    nb_lignes = len(lignes) - 1
    print(f"{nb_articles} articles, {nb_lignes} lignes "
          f"({nb_lignes - nb_articles} conditionnements)", file=sys.stderr)


if __name__ == "__main__":
    main()
