// equipe/JoursTravailles.tsx — la grille du mois (PLAN-EQUIPE, F-3).
//
// Une case par personne et par jour : vide (pas su), P (présent),
// ½ (demi-journée), A (absent). Un clic ouvre un petit menu : présent,
// ou absent / demi-journée avec son motif (D52 : maladie, permission,
// congé, absence). Le numéro du jour marque « tout le monde présent »
// sans toucher aux absences. Le compte de la ligne est ce que la paie
// d'un journalier lira ; le motif, la paie ne le lit pas.

import { useState, useEffect, useCallback } from "react";
import { Loader2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";

interface Grille {
  mois: string;
  jours: string[];
  aujourd_hui: string;
  personnes: {
    employe_id: string; nom: string; fonction: string;
    date_entree: string | null; date_depart: string | null;
    etats: Record<string, string>; jours_travailles: number;
    /** D52 : le motif des jours A et ½ (un A sans motif dit vaut « absence »). */
    motifs?: Record<string, string>;
    /** Ses fiches de paie validées qui touchent le mois : leurs jours sont figés. */
    fiches_validees?: { du: string; au: string; numero: string | null }[];
  }[];
}

const MARQUE: Record<string, string> = { present: "P", demi: "½", absent: "A" };
/** D52 : les motifs d'un jour non travaillé, et leur lettre dans la case. */
const MOTIFS: { cle: string; libelle: string; lettre: string }[] = [
  { cle: "absence", libelle: "Absence", lettre: "A" },
  { cle: "maladie", libelle: "Maladie", lettre: "M" },
  { cle: "permission", libelle: "Permission", lettre: "Pe" },
  { cle: "conge", libelle: "Congé", lettre: "C" },
];
const motifDe = (cle: string | undefined) => MOTIFS.find(m => m.cle === (cle ?? "absence")) ?? MOTIFS[0];

/** Ce que la case affiche : P, ½, A, M, Pe, C ; ½ suivi de la lettre du motif s'il en a un. */
function marque(etat: string, motif: string | undefined): string {
  if (etat === "absent") return motifDe(motif).lettre;
  if (etat === "demi") return motif && motif !== "absence" ? `½${motifDe(motif).lettre}` : "½";
  return MARQUE[etat] ?? "";
}

/** La case ouverte : qui, quel jour, où poser le menu. */
interface Menu { employeId: string; nom: string; jour: string; etat: string; motif?: string; fige: boolean; x: number; y: number }
const COULEUR: Record<string, string> = {
  present: "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200",
  demi: "bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200",
  absent: "bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-200",
};

export function JoursTravailles() {
  const [mois, setMois] = useState(new Date().toISOString().slice(0, 7));
  const [g, setG] = useState<Grille | null>(null);
  const [avis, setAvis] = useState<string | null>(null);
  const [enCours, setEnCours] = useState(false);
  const [menu, setMenu] = useState<Menu | null>(null);
  const gere = peut("personnel:gerer");

  const charger = useCallback(async () => {
    try {
      setG(await invoke<Grille>("lire_presences_mois", { mois }));
    } catch (e) {
      setAvis(String(e));
    }
  }, [mois]);
  useEffect(() => { charger(); }, [charger]);

  async function faire(action: () => Promise<unknown>) {
    setMenu(null);
    setEnCours(true);
    setAvis(null);
    try {
      const r = await action() as { figes?: { nom: string; numero: string }[] } | null;
      // « Tous présents » ne touche pas qui a déjà ce jour sur une fiche
      // de paie validée (revue du 28/10) : on le dit.
      if (r?.figes?.length) {
        setAvis(`Pas marqué(s), ce jour est déjà sur une fiche de paie validée : ${r.figes.map(x => `${x.nom} (${x.numero})`).join(", ")}.`);
      }
      await charger();
    } catch (e) {
      setAvis(String(e));
    } finally {
      setEnCours(false);
    }
  }

  if (!g) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  const hors = (p: Grille["personnes"][number], j: string) =>
    j > g.aujourd_hui || (p.date_entree !== null && j < p.date_entree) || (p.date_depart !== null && j > p.date_depart);
  // Un jour payé par une fiche validée ne se change plus : une
  // rectificative de la fiche le rouvre (le serveur le refuse aussi).
  const fige = (p: Grille["personnes"][number], j: string) =>
    (p.fiches_validees ?? []).find(f => f.du <= j && j <= f.au);
  const weekend = (j: string) => [0, 6].includes(new Date(j + "T12:00:00").getDay());

  return (
    <div className="space-y-3" data-testid="jours-travailles">
      <div className="flex items-center gap-3">
        <input type="month" value={mois} onChange={e => setMois(e.target.value)} aria-label="Mois"
          className="h-8 px-2 text-sm border border-border rounded-md bg-background" />
        {enCours && <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />}
        <span className="text-xs text-muted-foreground">
          P présent · ½ demi-journée · A absence · M maladie · Pe permission · C congé · vide : pas su.
          {gere ? " Un clic sur une case : la choisir ; sur le numéro du jour : tout le monde présent." : ""}
        </span>
      </div>
      <p className="text-xs text-muted-foreground">
        Le motif ne change pas la paie : au prorata, chaque A ou ½ retire, congé compris. Un congé payé se marque P.
      </p>
      {avis && <p className="text-sm text-red-600" role="status">{avis}</p>}
      {g.personnes.length === 0 ? (
        <p className="text-sm text-muted-foreground">Personne dans ce mois.</p>
      ) : (
        <div className="overflow-x-auto border border-border rounded-lg">
          <table className="text-xs border-collapse">
            <thead>
              <tr>
                <th className="sticky left-0 bg-background text-left px-3 py-1.5 font-medium min-w-40">Personne</th>
                {g.jours.map(j => (
                  <th key={j} className={`px-0.5 py-1.5 font-normal ${weekend(j) ? "text-muted-foreground" : ""}`}>
                    {gere && j <= g.aujourd_hui ? (
                      <button className="w-6 hover:underline" disabled={enCours} aria-label={`Tous présents le ${j}`}
                        onClick={() => faire(() => invoke("marquer_tous_presents", { jour: j }))}>
                        {Number(j.slice(8))}
                      </button>
                    ) : <span className="inline-block w-6">{Number(j.slice(8))}</span>}
                  </th>
                ))}
                <th className="px-3 py-1.5 font-medium">Jours</th>
              </tr>
            </thead>
            <tbody>
              {g.personnes.map(p => (
                <tr key={p.employe_id} className="border-t border-border" data-testid="ligne-jours">
                  <td className="sticky left-0 bg-background px-3 py-1">
                    <div className="font-medium text-sm">{p.nom}</div>
                    <div className="text-muted-foreground">{p.fonction}</div>
                  </td>
                  {g.jours.map(j => {
                    const etat = p.etats[j] ?? "";
                    const motif = p.motifs?.[j];
                    const paye = fige(p, j);
                    // D52 : sur un jour figé, le motif d'un A ou ½ se corrige
                    // encore (la paie ne le lit pas) ; l'état, non.
                    const motifSeul = !!paye && (etat === "absent" || etat === "demi");
                    const interdit = hors(p, j) || (!!paye && !motifSeul);
                    const dit = etat === "absent" ? `absent, ${motifDe(motif).libelle.toLowerCase()}`
                      : etat === "demi" ? `demi-journée${motif && motif !== "absence" ? `, ${motifDe(motif).libelle.toLowerCase()}` : ""}`
                      : etat === "present" ? "présent" : "pas su";
                    return (
                      <td key={j} className={`px-0.5 py-1 text-center ${weekend(j) ? "bg-muted/40" : ""}`}>
                        <button
                          disabled={!gere || interdit || enCours}
                          aria-label={`${p.nom} le ${j} : ${dit}`}
                          aria-haspopup="menu"
                          title={paye ? `Sur la fiche ${paye.numero ?? ""} validée : ${motifSeul ? "seul le motif se change" : "faire une rectificative pour le changer"}` : interdit ? "Hors de ses dates" : dit}
                          onClick={ev => {
                            const r = ev.currentTarget.getBoundingClientRect();
                            setMenu({ employeId: p.employe_id, nom: p.nom, jour: j, etat, motif, fige: motifSeul, x: r.left, y: r.bottom + 4 });
                          }}
                          className={`min-w-6 h-6 px-0.5 rounded text-[11px] font-semibold ${etat ? COULEUR[etat] : "border border-border"} ${interdit ? "opacity-30" : ""}`}>
                          {marque(etat, motif)}
                        </button>
                      </td>
                    );
                  })}
                  <td className="px-3 py-1 text-right text-sm font-semibold tabular-nums" data-testid="total-jours">
                    {p.jours_travailles.toLocaleString("fr-FR")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {menu && <MenuCase menu={menu} onFermer={() => setMenu(null)}
        onChoisir={(etat, motif) => faire(() => invoke("marquer_presence", { employeId: menu.employeId, jour: menu.jour, etat, motif }))} />}
    </div>
  );
}

/** Le petit menu d'une case : présent, absent ou demi-journée avec son
 * motif, effacer. Sur un jour figé par une fiche validée, seuls les
 * motifs de l'état du jour s'offrent. */
function MenuCase({ menu, onChoisir, onFermer }: {
  menu: Menu;
  onChoisir: (etat: string | null, motif: string | null) => void;
  onFermer: () => void;
}) {
  useEffect(() => {
    const touche = (e: KeyboardEvent) => { if (e.key === "Escape") onFermer(); };
    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  }, [onFermer]);
  const actuel = (etat: string, motif: string) => menu.etat === etat && (menu.motif ?? "absence") === motif;
  const puce = "h-7 px-2 rounded-md border text-xs";
  const ligne = (etat: "absent" | "demi", libelle: string) => (
    <div className="flex items-center gap-1 flex-wrap">
      <span className="w-24 text-xs text-muted-foreground">{libelle}</span>
      {MOTIFS.map(m => (
        <button key={m.cle} role="menuitem" aria-pressed={actuel(etat, m.cle)} onClick={() => onChoisir(etat, m.cle)}
          className={`${puce} ${actuel(etat, m.cle) ? `${COULEUR[etat]} border-transparent font-semibold` : "border-border hover:bg-muted"}`}>
          {m.libelle}
        </button>
      ))}
    </div>
  );
  return (
    <>
      <div className="fixed inset-0 z-40" onClick={onFermer} aria-hidden="true" />
      <div role="menu" aria-label={`${menu.nom} le ${menu.jour}`} data-testid="menu-case"
        style={{ left: Math.max(8, Math.min(menu.x, window.innerWidth - 360)), top: Math.min(menu.y, window.innerHeight - 170) }}
        className="fixed z-50 w-[22rem] max-w-[calc(100vw-16px)] rounded-lg border border-border bg-card p-3 shadow-lg space-y-2">
        <div className="text-xs font-medium">{menu.nom} — {menu.jour.split("-").reverse().join("/")}</div>
        {menu.fige ? (
          <>
            {ligne(menu.etat as "absent" | "demi", menu.etat === "absent" ? "Absent" : "Demi-journée")}
            <p className="text-xs text-muted-foreground">Jour sur une fiche de paie validée : seul le motif se change.</p>
          </>
        ) : (
          <>
            <div className="flex items-center gap-1">
              <span className="w-24 text-xs text-muted-foreground">Travaillé</span>
              <button role="menuitem" aria-pressed={menu.etat === "present"} onClick={() => onChoisir("present", null)}
                className={`${puce} ${menu.etat === "present" ? `${COULEUR.present} border-transparent font-semibold` : "border-border hover:bg-muted"}`}>
                Présent
              </button>
            </div>
            {ligne("absent", "Absent")}
            {ligne("demi", "Demi-journée")}
            {menu.etat && (
              <button role="menuitem" onClick={() => onChoisir(null, null)} className="text-xs text-muted-foreground underline">
                Effacer (pas su)
              </button>
            )}
          </>
        )}
      </div>
    </>
  );
}
