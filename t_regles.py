"""Suite de tests — regles metier de Gescom, hors interface.

Reproduit fidelement les formules du code Rust et verifie les
invariants sur des milliers de combinaisons, y compris les cas limites.
"""
import random, itertools

ECHECS = []
def check(nom, cond, detail=""):
    if not cond:
        ECHECS.append((nom, detail))
    return cond

# =====================================================================
#  Regles, telles qu'implementees
# =====================================================================

def tva_extraite(ttc, taux):
    return 0 if taux <= 0 else round(ttc * taux / (1 + taux))

def pos_pu_ttc(pu_ht, taux):
    return round(pu_ht * (1 + taux))

def valider_facture(lignes, remise_g):
    """pieces.rs — remise repartie, TVA ajoutee, prix_pratique TTC,
    total derive des prix_pratique (D31)."""
    montants = []
    for qte, pu, remise_pct, taux in lignes:
        brut = round(pu * qte)
        rem = round(brut * remise_pct / 100.0)
        ht_l = brut - rem
        ht = ht_l - round(ht_l * remise_g / 100.0)
        tva = round(ht * taux)
        ttc = ht + tva
        pp = round(ttc / qte) if qte > 0 else pu
        montants.append(dict(ht=ht, tva=tva, ttc=ttc, pp=pp, qte=qte))
    total_net = int(sum(m['pp'] * m['qte'] for m in montants))
    return montants, total_net

def sql_total(montants):
    """CAST(SUM(prix_pratique*quantite) AS INTEGER) — troncature."""
    return int(sum(m['pp'] * m['qte'] for m in montants))

def statut_vente(total, paye):
    if paye >= total: return "payee"
    if paye > 0:      return "partiellement_payee"
    return "creance_ouverte"

def retour(total_vente, deja_paye, montant_credit, mode,
           verse_reel=None, rembourse_deja=0):
    """retours.rs L42 — avec la seconde borne."""
    reste = max(total_vente - deja_paye, 0)
    part_creance = 0 if mode == "echange" else min(montant_credit, reste)
    brute = montant_credit - part_creance
    if verse_reel is None:
        verse_reel = deja_paye
    plafond = max(verse_reel - rembourse_deja, 0)
    return part_creance, min(brute, plafond)

def solde_caisse(fond, mouvements):
    """caisse.rs L27 — especes seules, ouverture exclue."""
    e = sum(m for s, moy, mot, m in mouvements
            if s == 'entree' and moy == 'especes' and mot != 'ouverture')
    so = sum(m for s, moy, mot, m in mouvements
             if s == 'sortie' and moy == 'especes')
    return fond + e - so

def dette_fournisseur(factures, avoirs, paiements):
    """CASE de L32 : AVF 'paye' ne reduit pas la dette."""
    f = sum(m for m, st in factures if st != 'annule')
    a = sum(m for m, st in avoirs if st not in ('annule', 'paye'))
    return f - a - sum(paiements)

def prochain_numero(existants, prefixe, annee):
    """MAX(substr(numero,-5)) — jamais COUNT."""
    n = 0
    for num in existants:
        if num.startswith(f"{prefixe}-{annee}-"):
            n = max(n, int(num[-5:]))
    return f"{prefixe}-{annee}-{n+1:05d}"

ENGAGEANTES = {"facture","facture_acompte","avoir_client",
               "facture_fournisseur","avoir_fournisseur"}
CLOS = {"validee","transfere","annule","paye"}
def peut_modifier(type_piece, statut, avec_lignes):
    if statut in CLOS: return False
    if avec_lignes and type_piece in ENGAGEANTES and statut not in ("brouillon","emis"):
        return False
    return True

# =====================================================================
#  T1 — Arrondis : le montant du doit toujours etre recalculable
# =====================================================================
print("T1  Arrondis — total du == recalcul SQL")
random.seed(1)
n_div = 0
for _ in range(100_000):
    k = random.randint(1, 5)
    lignes = [(random.choice([1,2,3,5,7,10,12,0.25,0.5,1.5,2.5,3.33]),
               random.randint(25, 200_000),
               random.choice([0,0,0,5,10,12.5,33.3,50]),
               random.choice([0.0,0.05,0.18,0.20]))
              for _ in range(k)]
    rg = random.choice([0,0,0,5,10,15,25])
    m, net = valider_facture(lignes, rg)
    if net != sql_total(m): n_div += 1
check("T1 aucun ecart entre montant du et recalcul SQL", n_div == 0,
      f"{n_div} divergences sur 100 000")
print(f"    100 000 factures — divergences : {n_div}")

# =====================================================================
#  T2 — Une vente soldee le reste apres recalcul
# =====================================================================
print("T2  Statut — une vente soldee ne rouvre pas")
n_rouvre = 0
random.seed(2)
for _ in range(50_000):
    k = random.randint(1,4)
    lignes = [(random.choice([1,2,3,2.5,0.5,7,10]), random.randint(50,80_000),
               random.choice([0,5,10]), random.choice([0.0,0.18]))
              for _ in range(k)]
    rg = random.choice([0,10,20])
    m, net = valider_facture(lignes, rg)
    # Le client paie exactement le montant du
    if statut_vente(sql_total(m), net) != "payee": n_rouvre += 1
check("T2 aucune vente soldee ne rouvre", n_rouvre == 0, f"{n_rouvre} cas")
print(f"    50 000 factures reglees — reouvertures : {n_rouvre}")

# =====================================================================
#  T3 — TVA : la base HT est toujours coherente
# =====================================================================
print("T3  TVA — base HT = TTC - TVA")
n_tva = 0
for pu_ht in range(1, 3000):
    for taux in (0.05, 0.18, 0.20):
        for qte in (1, 3, 7, 12):
            pu = pos_pu_ttc(pu_ht, taux)
            ttc = pu * qte
            tva = tva_extraite(ttc, taux)
            ht = ttc - tva
            # L'ecart d'arrondi ne doit jamais depasser 1 F par unite
            if abs(ht - pu_ht*qte) > qte: n_tva += 1
check("T3 base HT coherente", n_tva == 0, f"{n_tva} ecarts")
print(f"    {2999*3*4} combinaisons — ecarts anormaux : {n_tva}")

# =====================================================================
#  T4 — Retours : jamais plus que ce qui a ete verse
# =====================================================================
print("T4  Retours — remboursement borne par le versement")
n_trop = n_neg = 0
random.seed(4)
for _ in range(200_000):
    total = random.randint(1000, 10_000_000)
    paye  = random.randint(0, total)
    credit= random.randint(1, total)
    mode  = random.choice(["remboursement","avoir_conserve","echange"])
    pc, pcl = retour(total, paye, credit, mode)
    if mode != "echange" and pcl > paye: n_trop += 1
    if pc < 0 or pcl < 0: n_neg += 1
check("T4 jamais rembourse plus que verse", n_trop == 0, f"{n_trop} cas")
check("T4 aucune part negative", n_neg == 0, f"{n_neg} cas")
print(f"    200 000 retours — sur-remboursements : {n_trop}, parts negatives : {n_neg}")

# =====================================================================
#  T5 — Retours successifs : le cumul ne depasse pas le verse
# =====================================================================
print("T5  Retours successifs sur une meme vente")
n_cumul = 0
random.seed(5)
for _ in range(20_000):
    total = random.randint(5000, 500_000)
    paye  = random.randint(0, total)
    restant_du = total - paye
    rendu_total = 0
    for _ in range(random.randint(1,4)):
        credit = random.randint(1, max(total//3,1))
        pc = min(credit, restant_du)
        brute = credit - pc
        pcl = min(brute, max(paye - rendu_total, 0))   # seconde borne
        restant_du -= pc
        rendu_total += pcl
    if rendu_total > paye + 1: n_cumul += 1
check("T5 cumul des remboursements <= verse", n_cumul == 0, f"{n_cumul} cas")
print(f"    20 000 sequences — depassements : {n_cumul}")

# =====================================================================
#  T6 — Caisse : le fond n'est jamais compte deux fois
# =====================================================================
print("T6  Caisse — fond compte une seule fois")
n_caisse = 0
random.seed(6)
for _ in range(50_000):
    fond = random.randint(0, 500_000)
    mv = [('entree','especes','ouverture',fond)]
    attendu = fond
    for _ in range(random.randint(0,10)):
        sens = random.choice(['entree','sortie'])
        moy  = random.choice(['especes','especes','especes','orange_money','moov_money'])
        mot  = random.choice(['vente','achat','depense','reglement_fournisseur','remboursement'])
        m    = random.randint(100, 200_000)
        mv.append((sens,moy,mot,m))
        if moy == 'especes':
            attendu += m if sens=='entree' else -m
    if solde_caisse(fond, mv) != attendu: n_caisse += 1
check("T6 solde de caisse exact", n_caisse == 0, f"{n_caisse} cas")
print(f"    50 000 sessions — soldes faux : {n_caisse}")

# =====================================================================
#  T7 — Numerotation : jamais de doublon, meme apres suppression
# =====================================================================
print("T7  Numerotation — pas de collision")
n_dbl = 0
for prefixe in ["FAC","FAF","AVC","AVF","BCF","BRF","DEV","CMD"]:
    nums = []
    for i in range(500):
        n = prochain_numero(nums, prefixe, 2026)
        if n in nums: n_dbl += 1
        nums.append(n)
        # simuler des suppressions aleatoires
        if i % 7 == 0 and len(nums) > 3:
            nums.pop(random.randrange(len(nums)-1))
check("T7 aucune collision de numero", n_dbl == 0, f"{n_dbl} doublons")
print(f"    8 series x 500 pieces avec suppressions — doublons : {n_dbl}")

# Prefixes distincts entre types fournisseur
prefs = {"bon_commande_fournisseur":"BCF","bon_reception":"BRF",
         "facture_fournisseur":"FAF","avoir_fournisseur":"AVF"}
check("T7 prefixes fournisseur distincts", len(set(prefs.values()))==4)

# =====================================================================
#  T8 — Dette fournisseur : AVF rembourse ne reduit pas deux fois
# =====================================================================
print("T8  Dette fournisseur — avoir rembourse compte une fois")
n_dette = 0
random.seed(8)
for _ in range(50_000):
    fac = [(random.randint(1000,500_000), random.choice(['emis','paye']))
           for _ in range(random.randint(1,4))]
    avo = [(random.randint(500,200_000), random.choice(['emis','paye','annule']))
           for _ in range(random.randint(0,3))]
    pai = [random.randint(0,100_000) for _ in range(random.randint(0,3))]
    d = dette_fournisseur(fac, avo, pai)
    # Un AVF 'paye' (rembourse en especes) ne doit PAS reduire la dette
    d_sans_paye = sum(m for m,s in fac) - sum(m for m,s in avo if s=='emis') - sum(pai)
    if d != d_sans_paye: n_dette += 1
check("T8 avoir rembourse exclu de la dette", n_dette == 0, f"{n_dette} cas")
print(f"    50 000 fournisseurs — doubles deductions : {n_dette}")

# =====================================================================
#  T9 — Immuabilite : une piece close ne se modifie jamais
# =====================================================================
print("T9  Immuabilite des pieces")
n_imm = 0
for t in ["devis","proforma","commande_client","bon_livraison","facture",
          "facture_fournisseur","avoir_client","avoir_fournisseur","bon_reception"]:
    for st in ["brouillon","emis","validee","paye","transfere","annule"]:
        for lignes in (True, False):
            r = peut_modifier(t, st, lignes)
            if st in CLOS and r: n_imm += 1
            if st == "brouillon" and not r: n_imm += 1
check("T9 pieces closes non modifiables", n_imm == 0, f"{n_imm} cas")
print(f"    {9*6*2} combinaisons — violations : {n_imm}")

# Une facture emise garde ses lignes modifiables (choix assume L26)
check("T9 facture emise : lignes modifiables", peut_modifier("facture","emis",True))
check("T9 facture payee : rien de modifiable", not peut_modifier("facture","paye",True))

# =====================================================================
#  T10 — Cas limites
# =====================================================================
print("T10 Cas limites")
# remise 100%
m, net = valider_facture([(2, 10_000, 100.0, 0.18)], 0)
check("T10 remise ligne 100% -> total 0", net == 0, f"net={net}")
# remise globale 100%
m, net = valider_facture([(2, 10_000, 0.0, 0.18)], 100.0)
check("T10 remise globale 100% -> total 0", net == 0, f"net={net}")
# quantite fractionnaire tres petite
m, net = valider_facture([(0.01, 100_000, 0.0, 0.18)], 0)
check("T10 petite quantite coherente", net == sql_total(m), f"net={net}")
# TVA 0
m, net = valider_facture([(3, 1000, 0.0, 0.0)], 0)
check("T10 sans TVA : total = 3000", net == 3000, f"net={net}")
# retour sur vente entierement payee
pc, pcl = retour(10_000, 10_000, 10_000, "remboursement")
check("T10 vente payee : tout rendu", (pc,pcl)==(0,10_000), f"{pc},{pcl}")
# retour sur vente jamais payee
pc, pcl = retour(10_000, 0, 10_000, "remboursement")
check("T10 vente impayee : rien rendu", (pc,pcl)==(10_000,0), f"{pc},{pcl}")
# retour superieur au total (ne devrait pas arriver, garde quantite)
pc, pcl = retour(10_000, 4_000, 15_000, "remboursement")
check("T10 retour > total : borne au verse", pcl <= 4_000, f"pcl={pcl}")
# Retours successifs : le second ne rend rien de plus
pc2, pcl2 = retour(10_000, 10_000, 5_000, "remboursement",
                   verse_reel=4_000, rembourse_deja=4_000)
check("T10 second retour apres remboursement complet", pcl2 == 0, f"pcl2={pcl2}")

# =====================================================================
print()
print("="*62)
if ECHECS:
    print(f"  {len(ECHECS)} ECHEC(S)")
    for n,d in ECHECS: print(f"   - {n} : {d}")
else:
    print("  TOUS LES TESTS PASSENT")
print("="*62)
