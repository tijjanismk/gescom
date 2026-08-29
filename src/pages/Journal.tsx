// pages/Journal.tsx — Journal de la journée
//
// Reprend la structure du cahier tenu sous Excel : détail des ventes,
// encaissements hors du jour, impayés, achats, retours, dépenses, et le
// récapitulatif du bas de feuille.
//
// La distinction qui compte : CA du jour (ventes émises aujourd'hui,
// payées ou non) et ENCAISSÉ du jour (argent reçu aujourd'hui, y
// compris sur des ventes antérieures). Les additionner compterait deux
// fois une vente à crédit encaissée plus tard.

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  BookOpen, RefreshCw, Loader2, Printer, ChevronLeft, ChevronRight,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

// ---------------------------------------------------------------------

interface LigneVente {
  date: string; client: string; description: string; unite: string;
  prix_unitaire: number; quantite: number;
  montant_ttc: number; montant_ht: number;
  mode: string; numero: string | null;
}
interface HorsJour {
  client: string; montant: number; mode: string;
  date: string; date_vente: string; type: "solde" | "acompte";
}
interface Impaye {
  client: string; date_vente: string;
  total: number; paye: number; reste: number;
}
interface Achat {
  fournisseur: string; description: string;
  quantite: number; prix_unitaire: number; montant: number; date: string;
}
interface Retour {
  sens: "client" | "fournisseur"; tiers: string; description: string;
  quantite: number; prix_unitaire: number; montant: number; date: string;
}
interface Depense {
  libelle: string; categorie: string; moyen: string;
  montant: number; date: string;
}
interface Journal {
  date: string;
  ventes: LigneVente[];
  hors_jour: HorsJour[];
  impayes: Impaye[];
  achats: Achat[];
  retours: Retour[];
  depenses: Depense[];
  depenses_par_categorie: { categorie: string; montant: number }[];
  caisse_par_moyen: { moyen: string; entrees: number; sorties: number }[];
  totaux: {
    ca_jour: number; ca_jour_ht: number; encaisse_jour: number;
    hors_jour: number; impayes: number; achats: number;
    depenses: number; reglement_fournisseur: number; nb_ventes: number;
  };
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtHeure(iso: string): string {
  return new Date(iso).toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit",
  });
}
function fmtDateCourt(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit",
  });
}
function fmtQte(n: number): string {
  return n % 1 === 0 ? String(n) : n.toFixed(2);
}
function fmtCategorie(c: string): string {
  return {
    transport: "Transport", carburant: "Carburant", loyer: "Loyer",
    salaire: "Salaire", electricite: "Électricité", eau: "Eau",
    fourniture: "Fournitures", entretien: "Entretien", taxe: "Taxe",
    autre: "Autre",
  }[c] ?? c;
}
function fmtMoyen(m: string): string {
  return {
    especes: "Espèces", orange_money: "Orange Money",
    moov_money: "Moov Money", cheque: "Chèque", avoir: "Avoir",
  }[m] ?? m;
}

// Décalage de N jours sur une date AAAA-MM-JJ.
function decaler(dateIso: string, jours: number): string {
  const d = new Date(dateIso + "T12:00:00");
  d.setDate(d.getDate() + jours);
  return d.toISOString().split("T")[0];
}

// ---------------------------------------------------------------------

function Section({
  titre, couleur, children, vide,
}: {
  titre: string; couleur: string;
  children: React.ReactNode; vide?: boolean;
}) {
  if (vide) return null;
  return (
    <Card className="mb-4 overflow-hidden">
      <CardHeader className={`py-2 px-4 ${couleur}`}>
        <CardTitle className="text-sm font-bold tracking-wide uppercase">
          {titre}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0">{children}</CardContent>
    </Card>
  );
}

const TH = "text-left px-3 py-2 text-xs font-semibold text-muted-foreground";
const TD = "px-3 py-1.5 text-sm";

// ---------------------------------------------------------------------

export function Journal() {
  const aujourdhui = new Date().toISOString().split("T")[0];
  const [date, setDate] = useState(aujourdhui);
  const [data, setData] = useState<Journal | null>(null);
  const [chargement, setChargement] = useState(true);

  const charger = useCallback(async (d: string) => {
    setChargement(true);
    try {
      const j = await invoke<Journal>("lire_journal_du_jour", { date: d });
      setData(j);
    } catch (e) {
      console.error("Erreur journal :", e);
    } finally {
      setChargement(false);
    }
  }, []);

  useEffect(() => { charger(date); }, [date, charger]);

  function imprimer() { window.print(); }

  if (chargement && !data) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const t = data?.totaux;

  return (
    <div className="flex-1 overflow-auto p-6 print:p-0">

      {/* ── Barre d'outils (masquée à l'impression) ── */}
      <div className="flex flex-wrap items-center justify-between gap-3 mb-6 print:hidden">
        <div className="flex items-center gap-2">
          <BookOpen className="h-5 w-5" />
          <h1 className="text-2xl font-semibold">Journal</h1>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="icon"
            onClick={() => setDate(d => decaler(d, -1))}
            title="Jour précédent">
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Input type="date" value={date} max={aujourdhui}
            onChange={e => setDate(e.target.value)}
            className="h-9 w-40" />
          <Button variant="outline" size="icon"
            onClick={() => setDate(d => decaler(d, 1))}
            disabled={date >= aujourdhui}
            title="Jour suivant">
            <ChevronRight className="h-4 w-4" />
          </Button>
          <Button variant="outline" size="sm" onClick={() => charger(date)}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
          <Button size="sm" onClick={imprimer}>
            <Printer className="h-4 w-4 mr-2" /> Imprimer
          </Button>
        </div>
      </div>

      {/* ── En-tête du document ── */}
      <div className="text-center mb-5 py-2 bg-yellow-100 border border-yellow-300
                      rounded print:rounded-none">
        <h2 className="text-lg font-bold tracking-wide">
          JOURNAL DU {new Date(date + "T12:00:00")
            .toLocaleDateString("fr-ML", {
              day: "2-digit", month: "2-digit", year: "numeric",
            })}
        </h2>
      </div>

      {chargement && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
          <Loader2 className="h-4 w-4 animate-spin" /> Chargement…
        </div>
      )}

      {/* ── 1. Ventes du jour ── */}
      <Section titre={`Ventes du jour (${data?.ventes.length ?? 0})`}
        couleur="bg-slate-100" vide={!data?.ventes.length}>
        <table className="w-full">
          <thead className="border-b border-border">
            <tr>
              <th className={TH}>Heure</th>
              <th className={TH}>Client</th>
              <th className={TH}>Description</th>
              <th className={`${TH} text-right`}>P.U.</th>
              <th className={`${TH} text-right`}>Qté</th>
              <th className={`${TH} text-right`}>Montant</th>
              <th className={TH}>Règlement</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {data?.ventes.map((v, i) => (
              <tr key={i} className="hover:bg-muted/30">
                <td className={`${TD} text-muted-foreground`}>
                  {fmtHeure(v.date)}
                </td>
                <td className={TD}>{v.client}</td>
                <td className={TD}>
                  {v.description}
                  <span className="text-xs text-muted-foreground"> · {v.unite}</span>
                </td>
                <td className={`${TD} text-right`}>{fmt(v.prix_unitaire)}</td>
                <td className={`${TD} text-right`}>{fmtQte(v.quantite)}</td>
                <td className={`${TD} text-right font-semibold`}>
                  {fmt(v.montant_ttc)}
                </td>
                <td className={`${TD} text-xs text-muted-foreground`}>
                  {v.mode === "comptant" ? "Comptant" : "Crédit"}
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot className="border-t-2 border-foreground bg-muted/40">
            <tr>
              <td className={`${TD} font-bold`} colSpan={5}>TOTAL VENTES</td>
              <td className={`${TD} text-right font-bold`}>
                {fmt(t?.ca_jour ?? 0)}
              </td>
              <td />
            </tr>
          </tfoot>
        </table>
      </Section>

      {/* ── 2. Hors du jour ── */}
      <Section titre="Hors du jour — encaissements sur ventes antérieures"
        couleur="bg-blue-50" vide={!data?.hors_jour.length}>
        <table className="w-full">
          <thead className="border-b border-border">
            <tr>
              <th className={TH}>Client</th>
              <th className={TH}>Vente du</th>
              <th className={TH}>Nature</th>
              <th className={TH}>Moyen</th>
              <th className={`${TH} text-right`}>Montant</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {data?.hors_jour.map((h, i) => (
              <tr key={i} className="hover:bg-muted/30">
                <td className={TD}>{h.client}</td>
                <td className={`${TD} text-muted-foreground`}>
                  {fmtDateCourt(h.date_vente)}
                </td>
                <td className={TD}>
                  <span className={h.type === "solde"
                    ? "text-green-700" : "text-orange-600"}>
                    {h.type === "solde" ? "Solde" : "Acompte"}
                  </span>
                </td>
                <td className={`${TD} text-xs`}>{fmtMoyen(h.mode)}</td>
                <td className={`${TD} text-right font-semibold`}>
                  {fmt(h.montant)}
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot className="border-t-2 border-foreground bg-muted/40">
            <tr>
              <td className={`${TD} font-bold`} colSpan={4}>TOTAL HORS DU JOUR</td>
              <td className={`${TD} text-right font-bold`}>
                {fmt(t?.hors_jour ?? 0)}
              </td>
            </tr>
          </tfoot>
        </table>
      </Section>

      {/* ── 3. Achats ── */}
      <Section titre="Entrées / achats marchandises" couleur="bg-green-50"
        vide={!data?.achats.length}>
        <table className="w-full">
          <thead className="border-b border-border">
            <tr>
              <th className={TH}>Fournisseur</th>
              <th className={TH}>Description</th>
              <th className={`${TH} text-right`}>P.U.</th>
              <th className={`${TH} text-right`}>Qté</th>
              <th className={`${TH} text-right`}>Montant</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {data?.achats.map((a, i) => (
              <tr key={i} className="hover:bg-muted/30">
                <td className={TD}>{a.fournisseur}</td>
                <td className={TD}>{a.description}</td>
                <td className={`${TD} text-right`}>{fmt(a.prix_unitaire)}</td>
                <td className={`${TD} text-right`}>{fmtQte(a.quantite)}</td>
                <td className={`${TD} text-right font-semibold`}>
                  {fmt(a.montant)}
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot className="border-t-2 border-foreground bg-muted/40">
            <tr>
              <td className={`${TD} font-bold`} colSpan={4}>TOTAL ACHATS</td>
              <td className={`${TD} text-right font-bold`}>
                {fmt(t?.achats ?? 0)}
              </td>
            </tr>
          </tfoot>
        </table>
      </Section>

      {/* ── 4. Retours ── */}
      <Section titre="Retours marchandises" couleur="bg-purple-50"
        vide={!data?.retours.length}>
        <table className="w-full">
          <thead className="border-b border-border">
            <tr>
              <th className={TH}>Sens</th>
              <th className={TH}>Tiers</th>
              <th className={TH}>Description</th>
              <th className={`${TH} text-right`}>Qté</th>
              <th className={`${TH} text-right`}>Montant</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {data?.retours.map((r, i) => (
              <tr key={i} className="hover:bg-muted/30">
                <td className={`${TD} text-xs`}>
                  {r.sens === "client" ? "Retour client" : "Retour fournisseur"}
                </td>
                <td className={TD}>{r.tiers}</td>
                <td className={TD}>{r.description}</td>
                <td className={`${TD} text-right`}>{fmtQte(r.quantite)}</td>
                <td className={`${TD} text-right font-semibold`}>
                  {fmt(r.montant)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      {/* ── 5. Dépenses ── */}
      <Section titre="Dépenses" couleur="bg-red-50" vide={!data?.depenses.length}>
        <table className="w-full">
          <thead className="border-b border-border">
            <tr>
              <th className={TH}>Heure</th>
              <th className={TH}>Libellé</th>
              <th className={TH}>Poste</th>
              <th className={TH}>Moyen</th>
              <th className={`${TH} text-right`}>Montant</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {data?.depenses.map((d, i) => (
              <tr key={i} className="hover:bg-muted/30">
                <td className={`${TD} text-muted-foreground`}>
                  {fmtHeure(d.date)}
                </td>
                <td className={TD}>{d.libelle}</td>
                <td className={`${TD} text-xs`}>{fmtCategorie(d.categorie)}</td>
                <td className={`${TD} text-xs`}>{fmtMoyen(d.moyen)}</td>
                <td className={`${TD} text-right font-semibold`}>
                  {fmt(d.montant)}
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot className="border-t-2 border-foreground bg-muted/40">
            <tr>
              <td className={`${TD} font-bold`} colSpan={4}>TOTAL DÉPENSES</td>
              <td className={`${TD} text-right font-bold`}>
                {fmt(t?.depenses ?? 0)}
              </td>
            </tr>
          </tfoot>
        </table>
        {(data?.depenses_par_categorie.length ?? 0) > 1 && (
          <div className="px-4 py-2 border-t border-border flex flex-wrap gap-x-6 gap-y-1">
            {data?.depenses_par_categorie.map(c => (
              <span key={c.categorie} className="text-xs text-muted-foreground">
                {fmtCategorie(c.categorie)} : <strong>{fmt(c.montant)}</strong>
              </span>
            ))}
          </div>
        )}
      </Section>

      {/* ── 6. Situation non payée ── */}
      <Section titre="Situation non payée" couleur="bg-orange-50"
        vide={!data?.impayes.length}>
        <table className="w-full">
          <thead className="border-b border-border">
            <tr>
              <th className={TH}>Client</th>
              <th className={TH}>Vente du</th>
              <th className={`${TH} text-right`}>Total</th>
              <th className={`${TH} text-right`}>Payé</th>
              <th className={`${TH} text-right`}>Reste dû</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {data?.impayes.map((im, i) => (
              <tr key={i} className="hover:bg-muted/30">
                <td className={TD}>{im.client}</td>
                <td className={`${TD} text-muted-foreground`}>
                  {fmtDateCourt(im.date_vente)}
                </td>
                <td className={`${TD} text-right`}>{fmt(im.total)}</td>
                <td className={`${TD} text-right text-green-700`}>
                  {fmt(im.paye)}
                </td>
                <td className={`${TD} text-right font-semibold text-red-600`}>
                  {fmt(im.reste)}
                </td>
              </tr>
            ))}
          </tbody>
          <tfoot className="border-t-2 border-foreground bg-muted/40">
            <tr>
              <td className={`${TD} font-bold`} colSpan={4}>TOTAL IMPAYÉS</td>
              <td className={`${TD} text-right font-bold text-red-600`}>
                {fmt(t?.impayes ?? 0)}
              </td>
            </tr>
          </tfoot>
        </table>
      </Section>

      {/* ── 7. Récapitulatif ── */}
      <Card className="mb-4 overflow-hidden">
        <CardHeader className="py-2 px-4 bg-foreground">
          <CardTitle className="text-sm font-bold tracking-wide uppercase
                                text-background">
            Récapitulatif
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <table className="w-full">
            <tbody className="divide-y divide-border">
              <tr>
                <td className={`${TD} font-medium`}>
                  Chiffre d'affaires du jour
                  <span className="block text-xs text-muted-foreground font-normal">
                    Ventes émises aujourd'hui, payées ou non
                    · {t?.nb_ventes ?? 0} ligne(s)
                  </span>
                </td>
                <td className={`${TD} text-right font-bold text-base`}>
                  {fmt(t?.ca_jour ?? 0)}
                </td>
              </tr>
              <tr className="bg-blue-50/50">
                <td className={`${TD} font-medium`}>
                  Encaissé aujourd'hui
                  <span className="block text-xs text-muted-foreground font-normal">
                    Argent reçu, y compris sur ventes antérieures.
                    Ne s'additionne pas au CA.
                  </span>
                </td>
                <td className={`${TD} text-right font-bold text-base text-green-700`}>
                  {fmt(t?.encaisse_jour ?? 0)}
                </td>
              </tr>
              <tr>
                <td className={TD}>Dont hors du jour</td>
                <td className={`${TD} text-right`}>{fmt(t?.hors_jour ?? 0)}</td>
              </tr>
              <tr>
                <td className={TD}>Achats marchandises</td>
                <td className={`${TD} text-right text-red-600`}>
                  − {fmt(t?.achats ?? 0)}
                </td>
              </tr>
              <tr>
                <td className={TD}>Règlements fournisseur</td>
                <td className={`${TD} text-right text-red-600`}>
                  − {fmt(t?.reglement_fournisseur ?? 0)}
                </td>
              </tr>
              <tr>
                <td className={TD}>Dépenses</td>
                <td className={`${TD} text-right text-red-600`}>
                  − {fmt(t?.depenses ?? 0)}
                </td>
              </tr>
              <tr className="bg-orange-50/50">
                <td className={`${TD} font-medium`}>Impayés clients (cumul)</td>
                <td className={`${TD} text-right font-semibold text-red-600`}>
                  {fmt(t?.impayes ?? 0)}
                </td>
              </tr>
            </tbody>
          </table>

          {/* Caisse par moyen */}
          {(data?.caisse_par_moyen.length ?? 0) > 0 && (
            <div className="border-t-2 border-foreground">
              <p className="px-3 py-2 text-xs font-semibold uppercase
                            text-muted-foreground">
                Mouvements de caisse par moyen
              </p>
              <table className="w-full">
                <tbody className="divide-y divide-border">
                  {data?.caisse_par_moyen.map(c => (
                    <tr key={c.moyen}>
                      <td className={TD}>{fmtMoyen(c.moyen)}</td>
                      <td className={`${TD} text-right text-green-700`}>
                        + {fmt(c.entrees)}
                      </td>
                      <td className={`${TD} text-right text-red-600`}>
                        − {fmt(c.sorties)}
                      </td>
                      <td className={`${TD} text-right font-semibold`}>
                        {fmt(c.entrees - c.sorties)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* ── Signatures (impression) ── */}
      <div className="hidden print:grid grid-cols-3 gap-8 mt-12 text-center text-sm">
        <div className="border-t border-foreground pt-2">Le caissier</div>
        <div className="border-t border-foreground pt-2">Le gérant</div>
        <div className="border-t border-foreground pt-2">Le PDG</div>
      </div>

      {(!data?.ventes.length && !data?.achats.length && !data?.depenses.length) && (
        <p className="text-center text-sm text-muted-foreground py-10">
          Aucun mouvement enregistré ce jour.
        </p>
      )}
    </div>
  );
}
