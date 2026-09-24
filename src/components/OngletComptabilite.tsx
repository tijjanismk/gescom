// components/OngletComptabilite.tsx — le plan comptable (v3, E-1, D23).
//
// Le plan SYSCOHADA est commun à tous les dossiers ; on y ajoute des
// sous-comptes pour le dossier ouvert (« 4111 Client Coulibaly »).
// Gescom n'écrit aucune écriture : il présentera ses opérations au
// comptable, compte par compte (E-2, E-3). Ici, le vocabulaire.

import { useState, useEffect, useCallback, useMemo } from "react";
import { Loader2, Plus } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

interface Compte {
  numero: string;
  libelle: string;
  classe: number;
  parent: string | null;
  sous_compte: boolean;
}

const CLASSES: Record<number, string> = {
  1: "Ressources durables", 2: "Actif immobilisé", 3: "Stocks", 4: "Tiers",
  5: "Trésorerie", 6: "Charges", 7: "Produits",
};

export function OngletComptabilite() {
  const [plan, setPlan] = useState<Compte[] | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [recherche, setRecherche] = useState("");
  const [numero, setNumero] = useState("");
  const [libelle, setLibelle] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);

  const charger = useCallback(async () => {
    try {
      setPlan(await invoke<Compte[]>("lire_plan_comptable"));
    } catch (e) {
      setErreur(String(e));
    }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  const visibles = useMemo(() => {
    const q = recherche.trim().toLowerCase();
    if (!plan) return [];
    return q ? plan.filter(c => c.numero.startsWith(q) || c.libelle.toLowerCase().includes(q)) : plan;
  }, [plan, recherche]);

  async function ajouter() {
    setEnCours(true);
    setAvis(null);
    try {
      const r = await invoke<{ numero: string; libelle: string; parent: string }>(
        "ajouter_sous_compte", { numero, libelle });
      setAvis({ texte: `Sous-compte ${r.numero} « ${r.libelle} » ajouté sous ${r.parent}.` });
      setNumero(""); setLibelle("");
      setRecherche(r.numero.slice(0, 3));
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  if (erreur) return <p className="text-sm text-red-600 py-4">{erreur}</p>;
  if (!plan) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  const nbSous = plan.filter(c => c.sous_compte).length;

  return (
    <div className="space-y-5 max-w-3xl">
      <p className="text-sm text-muted-foreground">
        Le plan SYSCOHADA révisé ({plan.length - nbSous} comptes), commun à tous les dossiers.
        Les sous-comptes ajoutés ici n'appartiennent qu'au dossier ouvert.
      </p>

      <div className="border border-border rounded-lg p-4 space-y-3">
        <h3 className="text-sm font-semibold">Nouveau sous-compte</h3>
        <div className="flex gap-3 items-end flex-wrap">
          <label className="text-xs text-muted-foreground flex flex-col">
            Numéro
            <Input value={numero} onChange={e => setNumero(e.target.value)} aria-label="Numéro du sous-compte"
              placeholder="4111" className="mt-1 h-9 w-32" inputMode="numeric" />
          </label>
          <label className="text-xs text-muted-foreground flex-1 min-w-48 flex flex-col">
            Libellé
            <Input value={libelle} onChange={e => setLibelle(e.target.value)} aria-label="Libellé du sous-compte"
              placeholder="Client Coulibaly" className="mt-1 h-9" />
          </label>
          <Button size="sm" onClick={ajouter} disabled={enCours || !numero.trim() || !libelle.trim()}>
            {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : <><Plus className="h-4 w-4 mr-1" /> Ajouter</>}
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Il commence par un compte du plan : 411… pour un client, 401… pour un fournisseur, 521… pour une banque.
        </p>
        {avis && (
          <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>
        )}
      </div>

      <div className="space-y-2">
        <Input value={recherche} onChange={e => setRecherche(e.target.value)} aria-label="Chercher un compte"
          placeholder="Chercher : 411, caisse, TVA…" className="h-9 max-w-sm" />
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="plan-comptable">
          {visibles.map((c, i) => {
            const nouvelleClasse = i === 0 || visibles[i - 1].classe !== c.classe;
            return (
              <div key={c.numero + (c.sous_compte ? "-s" : "")}>
                {nouvelleClasse && (
                  <div className="px-4 py-1.5 bg-muted/50 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Classe {c.classe} — {CLASSES[c.classe]}
                  </div>
                )}
                <div className="flex items-center gap-3 px-4 py-1.5 text-sm" data-testid="compte"
                  style={{ paddingLeft: `${1 + Math.max(0, c.numero.length - 2) * 0.75}rem` }}>
                  <span className={`font-mono w-16 shrink-0 ${c.numero.length <= 2 ? "font-semibold" : ""}`}>{c.numero}</span>
                  <span className={c.numero.length <= 2 ? "font-semibold" : ""}>{c.libelle}</span>
                  {c.sous_compte && <Badge variant="secondary">ce dossier</Badge>}
                </div>
              </div>
            );
          })}
          {visibles.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun compte.</p>}
        </div>
      </div>
    </div>
  );
}
