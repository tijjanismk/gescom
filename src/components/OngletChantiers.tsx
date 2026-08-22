// OngletChantiers.tsx — TVA, Dettes fournisseur, Irrécouvrable, Expiration avoirs
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Loader2, Save, RefreshCw, AlertTriangle,
  CheckCircle2, Clock, Percent, Banknote, XCircle
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { message } from "@tauri-apps/plugin-dialog";

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric"
  });
}

// =====================================================================
//  Onglet TVA
// =====================================================================

interface ArticleTVA {
  id: string; nom: string; unite_base: string; taux_tva: number;
}

export function OngletTVA() {
  const [articles, setArticles] = useState<ArticleTVA[]>([]);
  const [chargement, setChargement] = useState(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [taux, setTaux] = useState<Record<string, string>>({});
  const [tauxGlobal, setTauxGlobal] = useState("18");
  const [appliqueEnCours, setAppliqueEnCours] = useState(false);
  const [succesGlobal, setSuccesGlobal] = useState(false);

  // Période résumé
  const today = new Date().toISOString().slice(0, 10);
  const debut_mois = today.slice(0, 8) + "01";
  const [dateDebut, setDateDebut] = useState(debut_mois);
  const [dateFin, setDateFin] = useState(today);
  const [resume, setResume] = useState<any>(null);
  const [chargementResume, setChargementResume] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<ArticleTVA[]>("lire_taux_tva");
      setArticles(data);
      const init: Record<string, string> = {};
      data.forEach(a => { init[a.id] = (a.taux_tva * 100).toFixed(0); });
      setTaux(init);
    } catch (e) { console.error(e); }
    finally { setChargement(false); }
  }

  async function chargerResume() {
    setChargementResume(true);
    try {
      const data = await invoke<any>("lire_resume_tva", {
        dateDebut: dateDebut + "T00:00:00",
        dateFin: dateFin + "T23:59:59",
      });
      setResume(data);
    } catch (e) { console.error(e); }
    finally { setChargementResume(false); }
  }

  useEffect(() => { charger(); chargerResume(); }, []);

  async function sauvegarder(articleId: string) {
    const val = parseFloat(taux[articleId] ?? "0");
    if (isNaN(val) || val < 0 || val > 100) return;
    setSaving(articleId);
    try {
      await invoke("sauvegarder_tva_article", {
        articleId, tauxTva: val / 100,
      });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setSaving(null); }
  }

  async function appliquerATous() {
    const val = parseFloat(tauxGlobal);
    if (isNaN(val) || val < 0 || val > 100) return;
    if (!window.confirm(`Appliquer TVA ${val}% à TOUS les articles ?`)) return;
    setAppliqueEnCours(true);
    try {
      // Sauvegarder pour chaque article
      for (const a of articles) {
        await invoke("sauvegarder_tva_article", {
          articleId: a.id, tauxTva: val / 100,
        });
      }
      // Mettre à jour l'état local
      const newTaux: Record<string, string> = {};
      articles.forEach(a => { newTaux[a.id] = val.toFixed(0); });
      setTaux(newTaux);
      setSuccesGlobal(true);
      setTimeout(() => setSuccesGlobal(false), 2000);
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setAppliqueEnCours(false); }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  return (
    <div className="space-y-6">

      {/* ── Taux global ── */}
      <div className="border border-primary/30 bg-primary/5 rounded-lg p-4 space-y-3">
        <div>
          <p className="text-sm font-semibold">Taux TVA global</p>
          <p className="text-xs text-muted-foreground mt-0.5">
            Applique le même taux à tous les articles en une seule action.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <div className="relative w-24">
            <Input
              type="number" min="0" max="100" step="1"
              value={tauxGlobal}
              onChange={e => setTauxGlobal(e.target.value)}
              className="h-9 text-sm pr-6 font-medium" />
            <span className="absolute right-2 top-2 text-xs text-muted-foreground">%</span>
          </div>
          <Button size="sm" onClick={appliquerATous}
            disabled={appliqueEnCours}
            className="gap-2">
            {appliqueEnCours
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : succesGlobal
              ? <CheckCircle2 className="h-4 w-4" />
              : null
            }
            {succesGlobal ? "Appliqué !" : `Appliquer à tous (${articles.length} articles)`}
          </Button>
        </div>
        <div className="flex gap-2 flex-wrap">
          {[0, 5, 18].map(t => (
            <button key={t}
              onClick={() => setTauxGlobal(t.toString())}
              className={`px-3 py-1 rounded-full text-xs border transition-colors ${
                tauxGlobal === t.toString()
                  ? "bg-primary text-primary-foreground border-primary"
                  : "border-border text-muted-foreground hover:border-foreground"
              }`}>
              {t === 0 ? "Exonéré (0%)" : `TVA ${t}%`}
            </button>
          ))}
        </div>
      </div>

      {/* Résumé TVA collectée */}
      <div className="border border-border rounded-lg p-4 space-y-3">
        <p className="text-sm font-medium">TVA collectée sur la période</p>
        <div className="flex gap-2 items-end flex-wrap">
          <div>
            <Label className="text-xs">Du</Label>
            <Input type="date" value={dateDebut}
              onChange={e => setDateDebut(e.target.value)}
              className="h-8 text-sm w-36" />
          </div>
          <div>
            <Label className="text-xs">Au</Label>
            <Input type="date" value={dateFin}
              onChange={e => setDateFin(e.target.value)}
              className="h-8 text-sm w-36" />
          </div>
          <Button size="sm" variant="outline" onClick={chargerResume}
            disabled={chargementResume}>
            {chargementResume
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : <RefreshCw className="h-4 w-4" />}
          </Button>
        </div>
        {resume && (
          <div className="space-y-1">
            {resume.par_taux?.length > 0
              ? resume.par_taux.map((t: any) => (
                <div key={t.taux} className="flex justify-between text-sm">
                  <span className="text-muted-foreground">
                    TVA {(t.taux * 100).toFixed(0)}%
                    <span className="ml-2 text-xs">(base {fmt(t.total_ht)})</span>
                  </span>
                  <span className="font-medium">{fmt(t.total_tva)}</span>
                </div>
              ))
              : <p className="text-sm text-muted-foreground">Aucune TVA collectée sur cette période</p>
            }
            {resume.par_taux?.length > 1 && (
              <div className="flex justify-between text-sm font-medium border-t border-border pt-1 mt-1">
                <span>Total TVA</span>
                <span>{fmt(resume.total_tva)}</span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Taux par article */}
      <div>
        <p className="text-sm font-medium mb-1">Taux TVA par article</p>
        <p className="text-xs text-muted-foreground mb-3">
          0% = article exonéré. Modifiable article par article.
        </p>
        <div className="space-y-1">
          {articles.map(a => (
            <div key={a.id}
              className="flex items-center gap-3 px-3 py-2 border border-border rounded-md
                         hover:bg-muted/30 group">
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium truncate">{a.nom}</p>
                <p className="text-xs text-muted-foreground">{a.unite_base}</p>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <div className="relative w-20">
                  <Input
                    type="number" min="0" max="100" step="1"
                    value={taux[a.id] ?? "0"}
                    onChange={e => setTaux(prev =>
                      ({ ...prev, [a.id]: e.target.value }))}
                    onBlur={() => sauvegarder(a.id)}
                    onKeyDown={e => e.key === "Enter" && sauvegarder(a.id)}
                    className="h-8 text-sm pr-6"
                  />
                  <span className="absolute right-2 top-1.5 text-xs text-muted-foreground">%</span>
                </div>
                {saving === a.id
                  ? <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                  : <CheckCircle2 className="h-4 w-4 text-green-500 opacity-0
                                             group-hover:opacity-100 transition-opacity" />
                }
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// =====================================================================
//  Onglet Dettes fournisseurs
// =====================================================================

interface DetteFournisseur {
  id: string; nom: string; telephone?: string;
  est_voisin: boolean;
  total_achats: number; total_paye: number; dette: number;
}

export function OngletDettes() {
  const [dettes, setDettes] = useState<DetteFournisseur[]>([]);
  const [chargement, setChargement] = useState(true);
  const [fournisseurActif, setFournisseurActif] = useState<DetteFournisseur | null>(null);
  const [montant, setMontant] = useState("");
  const [mode, setMode] = useState("especes");
  const [note, setNote] = useState("");
  const [enCours, setEnCours] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<DetteFournisseur[]>("lire_dettes_fournisseurs");
      setDettes(data);
    } catch (e) { console.error(e); }
    finally { setChargement(false); }
  }

  useEffect(() => { charger(); }, []);

  async function handleRegler() {
    if (!fournisseurActif || !montant) return;
    const val = parseInt(montant);
    if (isNaN(val) || val <= 0) return;
    setEnCours(true);
    try {
      await invoke("regler_dette_fournisseur", {
        fournisseurId: fournisseurActif.id,
        montant: val, mode,
        note: note || null,
      });
      setFournisseurActif(null);
      setMontant(""); setNote("");
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setEnCours(false); }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  const totalDettes = dettes.reduce((s, d) => s + d.dette, 0);

  return (
    <div className="space-y-4">
      {totalDettes > 0 && (
        <div className="bg-amber-50 border border-amber-200 rounded-lg px-4 py-3
                        flex items-center justify-between">
          <span className="text-sm font-medium text-amber-800">Total dettes fournisseurs</span>
          <span className="text-sm font-bold text-amber-900">{fmt(totalDettes)}</span>
        </div>
      )}

      <div className="space-y-2">
        {dettes.map(d => (
          <div key={d.id}
            className="flex items-center justify-between px-4 py-3
                       border border-border rounded-lg">
            <div>
              <div className="flex items-center gap-2">
                <p className="text-sm font-medium">{d.nom}</p>
                {d.est_voisin && (
                  <Badge variant="outline" className="text-xs">Voisin</Badge>
                )}
              </div>
              {d.telephone && (
                <p className="text-xs text-muted-foreground">{d.telephone}</p>
              )}
              <p className="text-xs text-muted-foreground">
                Achats: {fmt(d.total_achats)} · Payé: {fmt(d.total_paye)}
              </p>
            </div>
            <div className="flex items-center gap-3">
              <div className="text-right">
                <p className={`text-sm font-bold ${d.dette > 0 ? "text-destructive" : "text-green-600"}`}>
                  {d.dette > 0 ? fmt(d.dette) : "Soldé"}
                </p>
              </div>
              {d.dette > 0 && (
                <Button size="sm" variant="outline"
                  onClick={() => { setFournisseurActif(d); setMontant(d.dette.toString()); }}>
                  Régler
                </Button>
              )}
            </div>
          </div>
        ))}

        {dettes.length === 0 && (
          <p className="text-sm text-muted-foreground text-center py-6">
            Aucun fournisseur enregistré
          </p>
        )}
      </div>

      {/* Modal règlement */}
      <Dialog open={!!fournisseurActif} onOpenChange={() => setFournisseurActif(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Régler dette — {fournisseurActif?.nom}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3 pt-2">
            <div className="text-sm text-muted-foreground">
              Dette actuelle : <span className="font-medium text-foreground">
                {fournisseurActif ? fmt(fournisseurActif.dette) : ""}
              </span>
            </div>
            <div>
              <Label>Montant (F)</Label>
              <Input type="number" value={montant}
                onChange={e => setMontant(e.target.value)}
                className="mt-1" autoFocus />
            </div>
            <div>
              <Label>Mode de paiement</Label>
              <select value={mode} onChange={e => setMode(e.target.value)}
                className="w-full mt-1 h-9 rounded-md border border-input bg-background
                           px-3 text-sm">
                <option value="especes">Espèces</option>
                <option value="orange_money">Orange Money</option>
                <option value="moov_money">Moov Money</option>
                <option value="cheque">Chèque</option>
              </select>
            </div>
            <div>
              <Label>Note (optionnel)</Label>
              <Input value={note} onChange={e => setNote(e.target.value)}
                placeholder="Ex: facture n°..." className="mt-1" />
            </div>
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setFournisseurActif(null)}
                className="flex-1">Annuler</Button>
              <Button onClick={handleRegler}
                disabled={!montant || enCours} className="flex-1">
                {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Confirmer"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// =====================================================================
//  Onglet Irrécouvrable
// =====================================================================

interface Irrecouvrable {
  id: string; vente_id: string; motif: string;
  date_marque: string; client_nom: string;
  facture_num?: string; montant_perdu: number;
}

interface CreanceOuverte {
  vente_id?: string;
  id?: string;       // certaines versions retournent id au lieu de vente_id
  client_nom: string;
  facture_num?: string;
  numero_facture?: string; // alias possible
  reste: number;
  date_vente: string;
}

export function OngletIrrecouvrable() {
  const [irrécouvrables, setIrrécouvrables] = useState<Irrecouvrable[]>([]);
  const [creances, setCreances] = useState<CreanceOuverte[]>([]);
  const [chargement, setChargement] = useState(true);
  const [venteActif, setVenteActif] = useState<CreanceOuverte | null>(null);
  const [motif, setMotif] = useState("");
  const [enCours, setEnCours] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const [irr, cr] = await Promise.all([
        invoke<Irrecouvrable[]>("lire_irrecouvrable"),
        invoke<any[]>("lire_creances_ouvertes"),
      ]);
      setIrrécouvrables(irr);
      // Normaliser : certaines versions retournent id ou vente_id
      const normalisees: CreanceOuverte[] = cr.map(c => ({
        ...c,
        vente_id: c.vente_id ?? c.id ?? "",
        facture_num: c.facture_num ?? c.numero_facture,
      }));
      const irr_ids = new Set(irr.map(i => i.vente_id));
      setCreances(normalisees.filter(c => c.vente_id && !irr_ids.has(c.vente_id)));
    } catch (e) { console.error(e); }
    finally { setChargement(false); }
  }

  useEffect(() => { charger(); }, []);

  async function handleMarquer() {
    if (!venteActif || !motif.trim()) return;
    setEnCours(true);
    try {
      await invoke("marquer_irrecouvrable", {
        venteId: venteActif.vente_id,
        motif: motif.trim(),
      });
      setVenteActif(null); setMotif("");
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setEnCours(false); }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  const totalPerdu = irrécouvrables.reduce((s, i) => s + i.montant_perdu, 0);

  return (
    <div className="space-y-6">
      {/* Créances ouvertes — peut marquer irrécouvrable */}
      {creances.length > 0 && (
        <div>
          <p className="text-sm font-medium mb-2">Créances ouvertes</p>
          <div className="space-y-1">
            {creances.map(c => (
              <div key={c.vente_id ?? c.client_nom}
                className="flex items-center justify-between px-3 py-2
                           border border-border rounded-md">
                <div>
                  <p className="text-sm font-medium">{c.client_nom}</p>
                  <p className="text-xs text-muted-foreground">
                    {c.facture_num ?? c.vente_id?.slice(0, 8) ?? "—"} · {fmtDate(c.date_vente)}
                  </p>
                </div>
                <div className="flex items-center gap-3">
                  <span className="text-sm font-medium text-destructive">
                    {fmt(c.reste)}
                  </span>
                  <Button size="sm" variant="outline"
                    className="text-destructive hover:text-destructive"
                    onClick={() => setVenteActif(c)}>
                    <XCircle className="h-4 w-4 mr-1" />
                    Irrécouvrable
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Historique irrécouvrables */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <p className="text-sm font-medium">Créances irrécouvrables</p>
          {totalPerdu > 0 && (
            <span className="text-xs text-destructive font-medium">
              Total perdu : {fmt(totalPerdu)}
            </span>
          )}
        </div>
        {irrécouvrables.length === 0
          ? <p className="text-sm text-muted-foreground text-center py-4">Aucune créance irrécouvrable</p>
          : (
            <div className="space-y-1">
              {irrécouvrables.map(i => (
                <div key={i.id}
                  className="px-3 py-2 border border-destructive/20 bg-destructive/5 rounded-md">
                  <div className="flex justify-between">
                    <p className="text-sm font-medium">{i.client_nom}</p>
                    <span className="text-sm font-medium text-destructive">
                      {fmt(i.montant_perdu)}
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {i.facture_num ?? ""} · {fmtDate(i.date_marque)}
                  </p>
                  <p className="text-xs text-muted-foreground italic">{i.motif}</p>
                </div>
              ))}
            </div>
          )
        }
      </div>

      {/* Modal confirmation irrécouvrable */}
      <Dialog open={!!venteActif} onOpenChange={() => setVenteActif(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <AlertTriangle className="h-4 w-4" />
              Marquer irrécouvrable
            </DialogTitle>
          </DialogHeader>
          <div className="space-y-3 pt-2">
            <div className="bg-destructive/10 rounded-md px-3 py-2 text-sm">
              <p className="font-medium">{venteActif?.client_nom}</p>
              <p className="text-muted-foreground">Reste dû : {venteActif ? fmt(venteActif.reste) : ""}</p>
            </div>
            <p className="text-sm text-muted-foreground">
              Cette action est irréversible. La créance sera conservée dans les archives
              mais n'apparaîtra plus dans les créances actives.
            </p>
            <div>
              <Label>Motif *</Label>
              <Input value={motif} onChange={e => setMotif(e.target.value)}
                placeholder="Ex: client disparu, insolvabilité..."
                className="mt-1" autoFocus />
            </div>
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setVenteActif(null)} className="flex-1">
                Annuler
              </Button>
              <Button variant="destructive" onClick={handleMarquer}
                disabled={!motif.trim() || enCours} className="flex-1">
                {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Confirmer"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// =====================================================================
//  Onglet Expiration avoirs
// =====================================================================

export function OngletAvoirs() {
  const [config, setConfig] = useState({ active: false, duree_jours: 90 });
  const [chargement, setChargement] = useState(true);
  const [saving, setSaving] = useState(false);
  const [expiration, setExpiration] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<{ active: boolean; duree_jours: number }>(
        "lire_config_avoirs"
      );
      setConfig(data);
    } catch (e) { console.error(e); }
    finally { setChargement(false); }
  }

  useEffect(() => { charger(); }, []);

  async function handleSauvegarder() {
    if (config.duree_jours < 30) {
      await message("La durée minimale est de 30 jours", { title: "Attention", kind: "warning" });
      return;
    }
    setSaving(true);
    try {
      await invoke("sauvegarder_config_avoirs", {
        active: config.active,
        dureeJours: config.duree_jours,
      });
      await message("Configuration sauvegardée ✓", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setSaving(false); }
  }

  async function handleExpirer() {
    setExpiration(true);
    try {
      const nb = await invoke<number>("expirer_avoirs");
      if (nb > 0) {
        await message(`${nb} avoir(s) expiré(s)`, { title: "Expiration effectuée", kind: "info" });
      } else {
        await message("Aucun avoir à expirer", { title: "OK", kind: "info" });
      }
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setExpiration(false); }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  return (
    <div className="space-y-6 max-w-md">
      <div className="bg-muted rounded-lg p-4 text-sm text-muted-foreground">
        <p className="font-medium text-foreground mb-1">Expiration automatique des avoirs</p>
        <p>Les avoirs non utilisés après la durée configurée sont automatiquement
        expirés. Les avoirs expirés restent visibles dans l'historique.</p>
      </div>

      {/* Activer/désactiver */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium">Expiration automatique</p>
          <p className="text-xs text-muted-foreground">
            {config.active ? "Active" : "Désactivée"}
          </p>
        </div>
        <button
          onClick={() => setConfig(prev => ({ ...prev, active: !prev.active }))}
          className={`relative w-12 h-6 rounded-full transition-colors ${
            config.active ? "bg-primary" : "bg-muted-foreground/30"
          }`}>
          <span className={`absolute top-1 w-4 h-4 rounded-full bg-white shadow transition-transform ${
            config.active ? "translate-x-7" : "translate-x-1"
          }`} />
        </button>
      </div>

      {/* Durée */}
      <div>
        <Label>Durée de validité (jours)</Label>
        <div className="flex items-center gap-2 mt-1">
          <Input
            type="number" min="30" step="10"
            value={config.duree_jours}
            onChange={e => setConfig(prev =>
              ({ ...prev, duree_jours: parseInt(e.target.value) || 90 }))}
            className="w-28"
            disabled={!config.active}
          />
          <span className="text-sm text-muted-foreground">jours (min. 30)</span>
        </div>
      </div>

      <div className="flex gap-2">
        <Button onClick={handleSauvegarder} disabled={saving} className="flex-1">
          {saving
            ? <Loader2 className="h-4 w-4 animate-spin" />
            : <><Save className="h-4 w-4 mr-2" /> Sauvegarder</>
          }
        </Button>
        <Button variant="outline" onClick={handleExpirer}
          disabled={expiration}>
          {expiration
            ? <Loader2 className="h-4 w-4 animate-spin" />
            : <><Clock className="h-4 w-4 mr-2" /> Expirer maintenant</>
          }
        </Button>
      </div>
    </div>
  );
}