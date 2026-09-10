"""Inventaire de ce que PostgreSQL refuserait dans le SQL de Gescom.

On ne cherche pas a estimer : on compte, fichier par fichier, chaque
construction que SQLite accepte et que PostgreSQL rejette ou interprete
autrement. Le resultat est une liste de travail, pas une impression.
"""
import re, pathlib, collections, sys

RACINE = pathlib.Path(sys.argv[1])

# Les constructions qui posent probleme, et pourquoi.
MOTIFS = [
    ("placeholder ?N",        r"\?\d+",                       "s'ecrit $N en PostgreSQL"),
    ("placeholder ? nu",      r"(?<![\w?$])\?(?!\d)",         "positionnel implicite, inexistant en PG"),
    ("julianday(",            r"\bjulianday\s*\(",            "-> age()/EXTRACT(EPOCH ...)"),
    ("strftime(",             r"\bstrftime\s*\(",             "-> to_char()"),
    ("datetime(",             r"\bdatetime\s*\(",             "-> now() / to_timestamp()"),
    ("date('now')",           r"\bdate\s*\(\s*'now'",         "-> CURRENT_DATE"),
    ("CAST(.. AS INTEGER)",   r"CAST\s*\([^)]*AS\s+INTEGER\s*\)", "INTEGER n'est pas un type de cast PG usuel (int/bigint)"),
    ("substr(",               r"\bsubstr\s*\(",               "existe en PG, mais indices negatifs non"),
    ("substr negatif",        r"substr\s*\([^,]+,\s*-\d+",    "PG ne compte pas depuis la fin"),
    ("INSERT OR IGNORE",      r"INSERT\s+OR\s+IGNORE",        "-> ON CONFLICT DO NOTHING"),
    ("INSERT OR REPLACE",     r"INSERT\s+OR\s+REPLACE",       "-> ON CONFLICT DO UPDATE"),
    ("randomblob/hex",        r"randomblob|lower\s*\(\s*hex", "-> gen_random_uuid()"),
    ("IFNULL(",               r"\bIFNULL\s*\(",               "-> COALESCE()"),
    ("GROUP_CONCAT(",         r"\bGROUP_CONCAT\s*\(",         "-> string_agg()"),
    ("last_insert_rowid",     r"last_insert_rowid",           "-> RETURNING"),
    ("PRAGMA",                r"\bPRAGMA\b",                  "n'existe pas"),
    ("VACUUM",                r"\bVACUUM\b",                  "existe, mais VACUUM INTO non"),
    ("AUTOINCREMENT",         r"\bAUTOINCREMENT\b",           "-> GENERATED / serial"),
    ("rowid",                 r"(?<![\w_])rowid(?![\w_])",    "pas de rowid implicite"),
    ("|| concatenation",      r"\|\|",                        "identique — pour memoire"),
    ("division entiere",      r"/\s*100\b",                   "PG divise en entier aussi : verifier"),
    ("booleen 0/1",           r"=\s*[01]\b",                  "PG distingue boolean et integer"),
]

# On ne regarde que les chaines qui ressemblent a du SQL.
SQL = re.compile(
    r'"([^"\\]*(?:\\.[^"\\]*)*)"|r#"(.*?)"#', re.S)
EST_SQL = re.compile(
    r"\b(SELECT|INSERT|UPDATE|DELETE|CREATE|PRAGMA|VACUUM|ALTER)\b", re.I)

par_motif = collections.Counter()
par_fichier = collections.defaultdict(collections.Counter)
exemples = {}
n_requetes = 0

for f in sorted(RACINE.rglob("*.rs")):
    if "target" in f.parts or "tests" in f.parts:
        continue
    texte = f.read_text(encoding="utf-8", errors="replace")
    for m in SQL.finditer(texte):
        sql = m.group(1) or m.group(2) or ""
        if not EST_SQL.search(sql):
            continue
        n_requetes += 1
        rel = f.relative_to(RACINE).as_posix()
        for nom, motif, _ in MOTIFS:
            trouves = re.findall(motif, sql, re.I)
            if trouves:
                par_motif[nom] += len(trouves)
                par_fichier[rel][nom] += len(trouves)
                exemples.setdefault(nom, (rel, sql.strip()[:110]))

print(f"{n_requetes} requetes SQL trouvees dans {RACINE}\n")
print(f"{'construction':24} {'occurrences':>12}   remede")
print("-" * 92)
for nom, motif, remede in MOTIFS:
    n = par_motif[nom]
    if n:
        print(f"{nom:24} {n:>12}   {remede}")

print("\nFichiers les plus touches (hors placeholders, qui sont mecaniques) :")
mecanique = {"placeholder ?N", "placeholder ? nu", "|| concatenation",
             "booleen 0/1", "division entiere"}
score = {f: sum(v for k, v in c.items() if k not in mecanique)
         for f, c in par_fichier.items()}
for f, n in sorted(score.items(), key=lambda x: -x[1])[:12]:
    if n:
        detail = ", ".join(f"{k}×{v}" for k, v in par_fichier[f].items()
                           if k not in mecanique)
        print(f"  {n:>4}  {f}\n        {detail}")

print("\nUn exemple par construction non mecanique :")
for nom, _, _ in MOTIFS:
    if nom in exemples and nom not in mecanique:
        rel, ex = exemples[nom]
        print(f"  [{nom}] {rel}\n      {ex}")
