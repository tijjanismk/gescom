"""Genere les poignees du serveur a partir des facades Tauri.

Chaque facade dit tout ce qu'il faut : le nom de la commande, le module
du noyau qui l'execute, ses parametres et leurs types. Retaper ces
cent-cinquante signatures a la main serait long, et faux quelque part.

Le bloc genere vit entre deux marqueurs dans `serveur/src/socle.rs` :
relancer le script le remplace, sans toucher aux poignees ecrites a la
main au-dessus — celles qui demandent un traitement particulier.

Usage, depuis la racine du depot :

    python outils/generer_socle.py
"""

import pathlib
import re

RACINE = pathlib.Path(__file__).resolve().parent.parent / "src-tauri"

MARQUEUR = "// <<< POIGNEES GENEREES >>>"

# Une lecture ne demande aucune permission : le jeton suffit. Le reste
# est une ECRITURE et porte la permission de son domaine. La liste
# blanche de `portes` fait le tri — ce qui n'y figure pas est refuse a
# l'employe, ce qui est le defaut sur.
PREFIXES_LECTURE = (
    "lire_", "chercher_", "total_", "diagnostiquer_", "exporter_",
    "calculer_", "compter_",
)

PERMISSION = {
    "achats": "achats:creer",
    "auth": "utilisateurs:gerer",
    "avoirs": "avoirs:gerer",
    "caisse": "caisse:mouvementer",
    "catalogue": "articles:creer",
    "chantiers": "chantiers:gerer",
    "cheques": "cheques:gerer",
    "codebarre": "articles:creer",
    "creances": "creances:gerer",
    "depots": "depots:gerer",
    "fournisseurs": "fournisseurs:regler",
    "journal": "journal:lire",
    "livraisons": "livraisons:enregistrer",
    "pagination": "ventes:lire",
    "parametres": "parametres:modifier",
    "pieces": "pieces:creer",
    "pieces_pos": "ventes:creer",
    "rapports": "rapports:lire",
    "relances": "creances:gerer",
    "retours": "retours:creer",
    "sauvegarde": "sauvegarde:lancer",
    "societe": "parametres:modifier",
    "transferts": "stock:transferer",
    "ventes": "ventes:creer",
    "modeles": "modeles:gerer",
    "logo": "parametres:modifier",
}

# Types du langage ou de serde : ils n'appartiennent a aucun module.
CONNUS = {
    "String", "Option", "Vec", "i64", "i32", "u32", "u64", "f64", "f32",
    "bool", "usize", "Value", "serde_json", "HashMap", "std",
}


def qualifier(type_rust, module):
    """Prefixe les types definis dans le module porte.

    `Vec<LigneAchat>` vu depuis le serveur n'existe pas : la structure
    vit dans `gescom_noyau::achats`. Sans ce prefixe la poignee ne
    compile pas, et l'erreur ne dit pas ou trouver le type.
    """
    def remplacer(m):
        nom = m.group(1)
        if nom in CONNUS:
            return nom
        return module + "::" + nom

    return re.sub(r"([A-Z]\w*)", remplacer, type_rust)


def camel(s):
    morceaux = s.split("_")
    return morceaux[0] + "".join(x.capitalize() for x in morceaux[1:])


def lire_facades():
    """Ce que chaque facade Tauri revele de la fonction du noyau."""
    trouvees = []
    for f in sorted((RACINE / "src" / "commandes").glob("*.rs")):
        if f.stem == "mod":
            continue
        s = f.read_text(encoding="utf-8")
        motif = (
            r"#\[tauri::command\]\npub fn (\w+)\(\n(.*?)\n\) -> [^{]+\{\n(.*?)\n\}\n"
        )
        for m in re.finditer(motif, s, re.S):
            nom, params, corps = m.group(1), m.group(2), m.group(3)
            appel = re.search(r"gescom_noyau::(\w+)::(\w+)\(&(mut )?conn", corps)
            if not appel:
                continue
            args = []
            for ligne in params.split("\n"):
                l = ligne.strip().rstrip(",")
                if not l or l.startswith("//"):
                    continue
                mm = re.match(r"(\w+)\s*:\s*(.+)$", l)
                if mm and "State<EtatApp>" not in mm.group(2):
                    args.append((mm.group(1), mm.group(2).strip()))
            trouvees.append({
                "nom": nom,
                "module": appel.group(1),
                "fonction": appel.group(2),
                "args": args,
                "fichier": f.stem,
            })
    return trouvees


def poignee(cmd):
    nom = cmd["nom"]
    module = cmd["module"]
    lecture = nom.startswith(PREFIXES_LECTURE)
    permission = PERMISSION.get(cmd["fichier"], "parametres:modifier")

    lignes = []
    valeurs = []
    for a, t in cmd["args"]:
        # Le role et l'identite viennent de la SESSION, jamais du JSON.
        # Un poste qui enverrait « patron » franchirait sinon les
        # controles reserves au patron.
        if a in ("utilisateur_role", "role"):
            if t.startswith("Option"):
                valeurs.append("Some(c.appelant.role.clone())")
            else:
                valeurs.append("c.appelant.role.clone()")
            continue
        if a in ("utilisateur_id", "auteur_id"):
            if t.startswith("Option"):
                valeurs.append("Some(c.appelant.utilisateur_id.clone())")
            else:
                valeurs.append("c.appelant.utilisateur_id.clone()")
            continue
        lignes.append(
            '        let {}: {} = arg(&p, "{}", "{}")?;'.format(
                a, qualifier(t, module), camel(a), a
            )
        )
        valeurs.append(a)

    corps_args = ("\n".join(lignes) + "\n") if lignes else ""
    appel_args = ", ".join(["c.conn"] + valeurs)
    nom_p = "p" if lignes else "_p"

    if lecture:
        tete = '    r.lecture("{}", |c, {}| {{\n'.format(nom, nom_p)
    else:
        tete = '    r.ecriture("{}", "{}", |c, {}| {{\n'.format(nom, permission, nom_p)

    return (
        tete
        + corps_args
        + "        let v = {}::{}({})?;\n".format(module, cmd["fonction"], appel_args)
        + "        serde_json::to_value(v).map_err(|e| e.to_string())\n"
        + "    });\n"
    )


def main():
    socle = RACINE / "serveur" / "src" / "socle.rs"
    s = socle.read_text(encoding="utf-8")

    # Ne compter comme ecrites a la main que les poignees HORS du bloc
    # genere : sinon, au deuxieme passage, le generateur se prend
    # lui-meme pour du travail manuel et ne regenere rien.
    hors = re.sub(
        re.escape(MARQUEUR) + ".*?" + re.escape(MARQUEUR), "", s, flags=re.S
    )
    deja = set(re.findall(r'r\.(?:lecture|ecriture)\("(\w+)"', hors))

    blocs = []
    modules = set()
    for cmd in lire_facades():
        if cmd["nom"] in deja:
            continue
        blocs.append(poignee(cmd))
        modules.add(cmd["module"])

    entete = (
        "    // =================================================================\n"
        "    //  Le reste du metier, genere depuis les facades Tauri\n"
        "    // =================================================================\n"
        "    //\n"
        "    //  Regenerer avec : python outils/generer_socle.py\n"
        "    //\n"
        "    //  Les lectures ne demandent aucune permission, le jeton suffit.\n"
        "    //  Les ecritures portent celle de leur domaine ; la liste blanche\n"
        "    //  de `portes` fait le tri.\n"
        "    //\n"
        "    //  Le role et l'identite viennent de la SESSION, jamais du JSON :\n"
        "    //  un poste qui enverrait « patron » franchirait sinon les\n"
        "    //  controles reserves au patron.\n\n"
    )
    genere = entete + "\n".join(blocs)

    if MARQUEUR in s:
        s = re.sub(
            re.escape(MARQUEUR) + ".*?" + re.escape(MARQUEUR),
            MARQUEUR + "\n" + genere + "    " + MARQUEUR,
            s, flags=re.S,
        )
    else:
        fin = "    r\n}"
        assert fin in s, "fin de registre() introuvable"
        s = s.replace(
            fin, MARQUEUR + "\n" + genere + "    " + MARQUEUR + "\n\n    r\n}", 1
        )

    m = re.search(r"use gescom_noyau::\{[^}]*\};", s)
    assert m, "import du noyau introuvable"
    actuels = set(re.findall(r"\w+", m.group(0)))
    actuels.discard("use")
    actuels.discard("gescom_noyau")
    tous = sorted(actuels | modules)
    s = (s[: m.start()]
         + "use gescom_noyau::{\n    " + ", ".join(tous) + ",\n};"
         + s[m.end():])

    socle.write_text(s, encoding="utf-8")
    print("{} poignees generees, {} ecrites a la main".format(len(blocs), len(deja)))


if __name__ == "__main__":
    main()
