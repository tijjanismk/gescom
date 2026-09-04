#!/usr/bin/env python3
"""Retire les imports inutilises signales par tsc (TS6133).

    npx tsc --noEmit > erreurs.txt 2>&1
    python3 nettoyer_imports.py erreurs.txt          # apercu
    python3 nettoyer_imports.py erreurs.txt --ecrire # applique

Ne touche QUE les lignes d'import. Les fonctions et constantes
inutilisees sont listees a part : elles demandent un jugement humain —
une fonction morte est parfois un bouton qu'on a oublie de brancher.
"""
import re
import sys
from pathlib import Path

ERREUR = re.compile(
    r"^(?P<fichier>[^\s(]+?):(?P<ligne>\d+):(?P<col>\d+) - error TS6133: "
    r"'(?P<nom>[^']+)'"
)


def bloc_import(lignes, i):
    """Bornes de l'instruction import contenant la ligne i (0-based).

    Un import multi-lignes ne commence pas forcement par `import` sur la
    ligne signalee par tsc — il faut remonter jusqu'a lui.
    """
    debut = i
    while debut >= 0 and not lignes[debut].lstrip().startswith("import"):
        debut -= 1
        if i - debut > 12:          # garde-fou : pas un import
            return None
    if debut < 0:
        return None
    fin = debut
    while fin < len(lignes) and ";" not in lignes[fin]:
        fin += 1
    if fin >= len(lignes):
        return None
    return debut, fin


def retirer(texte, nom):
    """Retire un specificateur nomme d'une instruction import."""
    # `Foo as Bar` : tsc signale le nom LOCAL (Bar).
    motif = re.compile(
        r"(?:[A-Za-z_$][\w$]*\s+as\s+)?\b" + re.escape(nom) + r"\b(?!\s*(?:as|from))"
    )
    if not motif.search(texte):
        return texte, False
    nouveau = motif.sub("", texte, count=1)
    # Nettoyer les virgules orphelines laissees par la suppression.
    nouveau = re.sub(r",\s*,", ",", nouveau)
    nouveau = re.sub(r"\{\s*,", "{", nouveau)
    nouveau = re.sub(r",\s*\}", " }", nouveau)
    nouveau = re.sub(r"[ \t]+\n", "\n", nouveau)
    return nouveau, True


def import_vide(texte):
    """L'import ne ramene plus rien : la ligne entiere peut partir."""
    return re.search(r"import\s*\{\s*\}\s*from", texte) is not None


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 1
    ecrire = "--ecrire" in sys.argv
    sortie = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")

    par_fichier = {}
    for l in sortie.splitlines():
        m = ERREUR.match(l.strip())
        if m:
            par_fichier.setdefault(m["fichier"], []).append(
                (int(m["ligne"]) - 1, m["nom"])
            )

    retires, manuels = 0, []

    for fichier, entrees in sorted(par_fichier.items()):
        chemin = Path(fichier)
        if not chemin.exists():
            print(f"  ?  {fichier} — introuvable")
            continue
        lignes = chemin.read_text(encoding="utf-8").splitlines(keepends=True)
        a_supprimer = set()
        modifie = False

        # Du bas vers le haut : les indices restent valides.
        for idx, nom in sorted(entrees, reverse=True):
            bloc = bloc_import(lignes, idx)
            if bloc is None:
                manuels.append((fichier, idx + 1, nom))
                continue
            debut, fin = bloc
            texte = "".join(lignes[debut:fin + 1])
            nouveau, trouve = retirer(texte, nom)
            if not trouve:
                manuels.append((fichier, idx + 1, nom))
                continue
            if import_vide(nouveau):
                a_supprimer.update(range(debut, fin + 1))
            else:
                lignes[debut:fin + 1] = nouveau.splitlines(keepends=True)
            print(f"  -  {fichier}: {nom}")
            retires += 1
            modifie = True

        if a_supprimer:
            lignes = [l for i, l in enumerate(lignes) if i not in a_supprimer]
        if modifie and ecrire:
            chemin.write_text("".join(lignes), encoding="utf-8")

    print(f"\n{retires} import(s) retire(s)"
          f"{'' if ecrire else ' — APERCU, rien ecrit (--ecrire pour appliquer)'}")

    if manuels:
        print("\nA traiter a la main (declarations, pas des imports) :")
        for f, l, n in manuels:
            print(f"  {f}:{l}  {n}")
        print("\n  Verifier AVANT de supprimer : une fonction inutilisee est\n"
              "  parfois un bouton jamais branche, pas du code mort.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
