"""Les icones de Gescom Serveur et de Gescom Equipe : le logo de Gescom
et une pastille en coin (choix du proprietaire, 28/10/2026).

La caisse garde l'icone d'origine (`src-tauri/icons/icon.png`). Le
serveur porte une pastille bleu nuit avec une tour, l'equipe une
pastille verte avec deux personnes. Le cercle blanc autour de la
pastille la detache du logo, bleu et vert lui aussi.

    python outils/icones/pastilles.py
    npx tauri icon outils/icones/serveur.png -o src-tauri/serveur-fenetre/icons
    npx tauri icon outils/icones/equipe.png  -o src-tauri/equipe/icons
"""
from pathlib import Path

from PIL import Image, ImageDraw

ICI = Path(__file__).resolve().parent
LOGO = ICI.parent.parent / "src-tauri" / "icons" / "icon.png"
TAILLE = 1024
BLANC = (255, 255, 255, 255)


def pastille(fond, dessiner):
    """Le logo, et en bas a droite une pastille de 46 % du cote."""
    logo = Image.open(LOGO).convert("RGBA").resize((TAILLE, TAILLE), Image.LANCZOS)
    calque = Image.new("RGBA", (TAILLE, TAILLE), (0, 0, 0, 0))
    d = ImageDraw.Draw(calque)
    cote = int(TAILLE * 0.46)
    x0, y0 = TAILLE - cote, TAILLE - cote
    d.ellipse((x0, y0, TAILLE - 1, TAILLE - 1), fill=BLANC)
    bord = int(cote * 0.07)
    d.ellipse((x0 + bord, y0 + bord, TAILLE - 1 - bord, TAILLE - 1 - bord), fill=fond)
    dessiner(d, x0 + cote / 2, y0 + cote / 2, cote)
    return Image.alpha_composite(logo, calque)


def tour(d, cx, cy, cote):
    """Deux tiroirs de serveur empiles, chacun avec son voyant."""
    l, h = cote * 0.44, cote * 0.17
    r = h * 0.25
    for i, dy in enumerate((-h * 0.62, h * 0.62)):
        haut = cy + dy - h / 2
        d.rounded_rectangle((cx - l / 2, haut, cx + l / 2, haut + h), radius=r, fill=BLANC)
        v = h * 0.32
        vx = cx - l / 2 + h * 0.55
        d.ellipse((vx - v / 2, haut + h / 2 - v / 2, vx + v / 2, haut + h / 2 + v / 2), fill=(30, 58, 138, 255))
        t = cx + l * 0.08
        d.rounded_rectangle((t, haut + h * 0.42, cx + l / 2 - h * 0.5, haut + h * 0.58), radius=1, fill=(30, 58, 138, 255))


def personnes(d, cx, cy, cote):
    """Deux personnes cote a cote : une tete, des epaules."""
    for dx, k in ((-cote * 0.11, 0.9), (cote * 0.11, 1.0)):
        tete = cote * 0.12 * k
        x = cx + dx
        haut = cy - cote * 0.17
        d.ellipse((x - tete, haut - tete, x + tete, haut + tete), fill=BLANC)
        l = cote * 0.17 * k
        d.pieslice((x - l, cy - cote * 0.02, x + l, cy + cote * 0.30), 180, 360, fill=BLANC)


if __name__ == "__main__":
    pastille((30, 58, 138, 255), tour).save(ICI / "serveur.png")
    pastille((21, 128, 61, 255), personnes).save(ICI / "equipe.png")
    print("serveur.png, equipe.png")
