import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Phone, MessageCircle, Mail, MapPin,
  Clock, Loader2, ChevronDown, ChevronRight,
  AlertTriangle, CheckCircle2, History,
  Filter, RefreshCw, X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";

// =====================================================================
//  Types
// =====================================================================

interface Creance {
  vente_id: string; date_vente: string; statut: string;
  client_id: string; client_nom: string; client_code: string;
  telephone?: string; facture_num?: string;
  total: number; total_paye: number; reste: number;
  jours_retard: number; nb_relances: number; derniere_relance?: string;
}

interface Relance {
  id: string; canal: string; note?: string;
  date_relance: string; auteur_nom?: string;
}

interface Stats {
  total_creances: number; sans_relance: number;
  relances_semaine: number; montant_en_jeu: number;
}

// =====================================================================
//  Utilitaires
// =====================================================================

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}
function fmtDateHeure(iso: string) {
  return new Date(iso).toLocaleString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
    hour: "2-digit", minute: "2-digit",
  });
}

const CANAUX = [
  { value: "whatsapp", label: "WhatsApp",  icone: MessageCircle, couleur: "text-green-600" },
  { value: "appel",    label: "Appel",     icone: Phone,         couleur: "text-blue-600"  },
  { value: "sms",      label: "SMS",       icone: MessageCircle, couleur: "text-purple-600"},
  { value: "email",    label: "Email",     icone: Mail,          couleur: "text-orange-600"},
  { value: "visite",   label: "Visite",    icone: MapPin,        couleur: "text-red-600"   },
];

function urgence(jours: number): { label: string; couleur: string } {
  if (jours > 90) return { label: "Critique",  couleur: "bg-red-100 text-red-700 border-red-200" };
  if (jours > 60) return { label: "Urgent",    couleur: "bg-orange-100 text-orange-700 border-orange-200" };
  if (jours > 30) return { label: "En retard", couleur: "bg-yellow-100 text-yellow-700 border-yellow-200" };
  return { label: "Récent", couleur: "bg-gray-100 text-gray-600 border-gray-200" };
}

// =====================================================================
//  Message WhatsApp
// =====================================================================

function genererMessageWhatsApp(
  creance: Creance, societe: string, personnalise: string
): string {
  if (personnalise) return personnalise;
  const msg = `Bonjour ${creance.client_nom},\n\n` +
    `Nous vous rappelons qu'une facture est en attente de règlement :\n` +
    `• Facture : ${creance.facture_num ?? creance.vente_id.slice(0, 8)}\n` +
    `• Montant : ${fmt(creance.reste)}\n` +
    `• Date : ${fmtDate(creance.date_vente)}\n\n` +
    `Merci de régulariser dans les meilleurs délais.\n\n` +
    `Cordialement,\n${societe}`;
  return msg;
}

// =====================================================================
//  Modal Relancer
// =====================================================================

function ModalRelancer({
  ouvert, creance, societe, onFermer, onRelance,
}: {
  ouvert: boolean; creance: Creance | null; societe: string;
  onFermer: () => void; onRelance: () => void;
}) {
  const [canal, setCanal] = useState("whatsapp");
  const [messagePerso, setMessagePerso] = useState("");
  const [note, setNote] = useState("");
  const [chargement, setChargement] = useState(false);
  const [envoye, setEnvoye] = useState(false);

  useEffect(() => {
    if (ouvert) {
      setCanal("whatsapp"); setMessagePerso("");
      setNote(""); setEnvoye(false);
    }
  }, [ouvert]);

  if (!creance) return null;

  const msgWA = genererMessageWhatsApp(creance, societe, messagePerso);

  function ouvrirWhatsApp() {
    if (!creance.telephone) return;
    const tel = creance.telephone.replace(/\D/g, "");
    const telMali = tel.startsWith("223") ? tel : `223${tel}`;
    const url = `https://wa.me/${telMali}?text=${encodeURIComponent(msgWA)}`;
    window.open(url, "_blank");
    setEnvoye(true);
  }

  function ouvrirSMS() {
    if (!creance.telephone) return;
    const tel = creance.telephone.replace(/\D/g, "");
    window.open(`sms:${tel}?body=${encodeURIComponent(msgWA)}`, "_blank");
    setEnvoye(true);
  }

  async function handleEnregistrer() {
    if (!creance) return;
    setChargement(true);
    try {
      await invoke("enregistrer_relance", {
        venteId: creance.vente_id,
        canal,
        note: note || (envoye ? `Message envoyé via ${canal}` : null),
      });
      onRelance();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <MessageCircle className="h-4 w-4 text-green-600" />
            Relancer — {creance.client_nom}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 pt-2">
          {/* Résumé créance */}
          <div className="bg-muted rounded-lg px-4 py-3 space-y-1">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">
                {creance.facture_num ?? "Créance"}
              </span>
              <span className="font-bold text-orange-600">{fmt(creance.reste)}</span>
            </div>
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>Depuis le {fmtDate(creance.date_vente)}</span>
              <span className="font-medium">
                {creance.jours_retard} jour{creance.jours_retard > 1 ? "s" : ""}
              </span>
            </div>
            {creance.nb_relances > 0 && (
              <p className="text-xs text-muted-foreground">
                {creance.nb_relances} relance{creance.nb_relances > 1 ? "s" : ""} déjà envoyée{creance.nb_relances > 1 ? "s" : ""}
                {creance.derniere_relance && ` · dernière le ${fmtDate(creance.derniere_relance)}`}
              </p>
            )}
          </div>

          {/* Canal */}
          <div>
            <Label className="text-xs mb-2 block">Canal de relance</Label>
            <div className="flex gap-2 flex-wrap">
              {CANAUX.map(c => {
                const Icone = c.icone;
                return (
                  <button key={c.value} onClick={() => setCanal(c.value)}
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full
                                text-xs font-medium border transition-colors ${
                      canal === c.value
                        ? "bg-primary text-primary-foreground border-primary"
                        : "border-border text-muted-foreground hover:border-foreground"
                    }`}>
                    <Icone className="h-3 w-3" />{c.label}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Message WhatsApp/SMS personnalisable */}
          {(canal === "whatsapp" || canal === "sms") && (
            <div>
              <div className="flex items-center justify-between mb-1.5">
                <Label className="text-xs">Message</Label>
                <button onClick={() => setMessagePerso("")}
                  className="text-xs text-muted-foreground hover:text-foreground">
                  Réinitialiser
                </button>
              </div>
              <textarea
                value={messagePerso || msgWA}
                onChange={e => setMessagePerso(e.target.value)}
                rows={6}
                className="w-full text-xs border border-border rounded-md p-3
                           bg-background resize-none focus:outline-none
                           focus:ring-1 focus:ring-primary"
              />
              {!creance.telephone && (
                <p className="text-xs text-orange-600 mt-1">
                  ⚠ Aucun numéro de téléphone pour ce client
                </p>
              )}
              <div className="flex gap-2 mt-2">
                {canal === "whatsapp" && (
                  <Button size="sm" onClick={ouvrirWhatsApp}
                    disabled={!creance.telephone}
                    className="flex-1 bg-green-600 hover:bg-green-700 gap-2">
                    <MessageCircle className="h-4 w-4" />
                    Ouvrir WhatsApp
                    {envoye && <CheckCircle2 className="h-4 w-4" />}
                  </Button>
                )}
                {canal === "sms" && (
                  <Button size="sm" onClick={ouvrirSMS}
                    disabled={!creance.telephone}
                    variant="outline" className="flex-1 gap-2">
                    <Phone className="h-4 w-4" />
                    Envoyer SMS
                    {envoye && <CheckCircle2 className="h-4 w-4 text-green-600" />}
                  </Button>
                )}
              </div>
            </div>
          )}

          {/* Note interne */}
          <div>
            <Label className="text-xs mb-1.5 block">Note interne (optionnel)</Label>
            <Input value={note} onChange={e => setNote(e.target.value)}
              placeholder="Ex: client rappellera demain matin..." />
          </div>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">
              Annuler
            </Button>
            <Button onClick={handleEnregistrer}
              disabled={chargement} className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Enregistrer la relance"
              }
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal Historique relances
// =====================================================================

function ModalHistorique({
  ouvert, creance, onFermer,
}: {
  ouvert: boolean; creance: Creance | null; onFermer: () => void;
}) {
  const [historique, setHistorique] = useState<Relance[]>([]);
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (!ouvert || !creance) return;
    setChargement(true);
    invoke<Relance[]>("lire_historique_relances", { venteId: creance.vente_id })
      .then(setHistorique)
      .catch(console.error)
      .finally(() => setChargement(false));
  }, [ouvert, creance]);

  if (!creance) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <History className="h-4 w-4" />
            Historique — {creance.client_nom}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">
          {chargement ? (
            <div className="flex justify-center py-6">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          ) : historique.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-6">
              Aucune relance enregistrée
            </p>
          ) : (
            historique.map(r => {
              const canal = CANAUX.find(c => c.value === r.canal);
              const Icone = canal?.icone ?? MessageCircle;
              return (
                <div key={r.id} className="flex gap-3">
                  <div className={`mt-0.5 shrink-0 ${canal?.couleur ?? "text-muted-foreground"}`}>
                    <Icone className="h-4 w-4" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-sm font-medium">
                        {canal?.label ?? r.canal}
                      </span>
                      <span className="text-xs text-muted-foreground shrink-0">
                        {fmtDateHeure(r.date_relance)}
                      </span>
                    </div>
                    {r.note && (
                      <p className="text-xs text-muted-foreground mt-0.5">{r.note}</p>
                    )}
                    {r.auteur_nom && (
                      <p className="text-xs text-muted-foreground">par {r.auteur_nom}</p>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page Relances
// =====================================================================

export function Relances() {
  const [creances, setCreances] = useState<Creance[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);
  const [chargement, setChargement] = useState(true);
  const [enRetardSeulement, setEnRetardSeulement] = useState(false);
  const [recherche, setRecherche] = useState("");
  const [triPar, setTriPar] = useState<"jours"|"montant"|"nb_relances">("jours");
  const [creanceActive, setCreanceActive] = useState<Creance | null>(null);
  const [modalRelancer, setModalRelancer] = useState(false);
  const [modalHistorique, setModalHistorique] = useState(false);
  const [expandeId, setExpandeId] = useState<string | null>(null);
  const societe = "Ma Boutique"; // TODO: charger depuis paramètres

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      const [cr, st] = await Promise.all([
        invoke<Creance[]>("lire_creances_relances", {
          enRetardSeulement: enRetardSeulement || null,
        }),
        invoke<Stats>("lire_stats_relances"),
      ]);
      setCreances(cr);
      setStats(st);
    } catch (e) { console.error(e); }
    finally { setChargement(false); }
  }, [enRetardSeulement]);

  useEffect(() => { charger(); }, [charger]);

  const creancesFiltrees = creances
    .filter(c => !recherche ||
      c.client_nom.toLowerCase().includes(recherche.toLowerCase()) ||
      c.telephone?.includes(recherche) ||
      c.facture_num?.includes(recherche)
    )
    .sort((a, b) => {
      switch (triPar) {
        case "jours":      return b.jours_retard - a.jours_retard;
        case "montant":    return b.reste - a.reste;
        case "nb_relances":return a.nb_relances - b.nb_relances;
        default:           return 0;
      }
    });

  const totalReste = creancesFiltrees.reduce((s, c) => s + c.reste, 0);

  return (
    <div className="flex-1 overflow-auto">
      <div className="p-6 space-y-5">

        {/* En-tête */}
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-semibold">Relances créances</h1>
          <Button size="sm" variant="outline" onClick={charger}>
            <RefreshCw className="h-3.5 w-3.5 mr-1" /> Actualiser
          </Button>
        </div>

        {/* KPIs */}
        {stats && (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {[
              { label: "Créances ouvertes", val: stats.total_creances.toString(),
                couleur: "text-orange-600", bg: "bg-orange-50 border-orange-200" },
              { label: "Sans relance", val: stats.sans_relance.toString(),
                couleur: "text-red-600", bg: "bg-red-50 border-red-200" },
              { label: "Relances cette semaine", val: stats.relances_semaine.toString(),
                couleur: "text-blue-600", bg: "bg-blue-50 border-blue-200" },
              { label: "Montant en jeu", val: fmt(stats.montant_en_jeu),
                couleur: "text-foreground", bg: "bg-muted border-border" },
            ].map(k => (
              <div key={k.label}
                className={`border rounded-xl px-4 py-3 ${k.bg}`}>
                <p className={`text-xl font-bold ${k.couleur}`}>{k.val}</p>
                <p className="text-xs text-muted-foreground mt-0.5">{k.label}</p>
              </div>
            ))}
          </div>
        )}

        {/* Filtres */}
        <div className="flex items-center gap-3 flex-wrap">
          <div className="relative">
            <Input value={recherche} onChange={e => setRecherche(e.target.value)}
              placeholder="Client, téléphone, facture..."
              className="h-8 text-sm w-52 pl-3" />
            {recherche && (
              <button onClick={() => setRecherche("")}
                className="absolute right-2 top-2 text-muted-foreground">
                <X className="h-3.5 w-3.5" />
              </button>
            )}
          </div>

          <button onClick={() => setEnRetardSeulement(!enRetardSeulement)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs
                        font-medium border transition-colors ${
              enRetardSeulement
                ? "bg-red-500 text-white border-red-500"
                : "border-border text-muted-foreground hover:border-foreground"
            }`}>
            <AlertTriangle className="h-3 w-3" />
            En retard (+30j)
          </button>

          <Select value={triPar} onValueChange={v => { if (v) setTriPar(v as any); }}>
            <SelectTrigger className="h-8 text-xs w-40">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="jours" className="text-xs">Ancienneté</SelectItem>
              <SelectItem value="montant" className="text-xs">Montant</SelectItem>
              <SelectItem value="nb_relances" className="text-xs">Moins relancés</SelectItem>
            </SelectContent>
          </Select>

          <div className="ml-auto text-sm">
            <span className="text-muted-foreground">
              {creancesFiltrees.length} créance{creancesFiltrees.length > 1 ? "s" : ""} ·{" "}
            </span>
            <span className="font-bold text-orange-600">{fmt(totalReste)}</span>
          </div>
        </div>

        {/* Liste créances */}
        {chargement ? (
          <div className="flex justify-center py-12">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : creancesFiltrees.length === 0 ? (
          <div className="text-center py-16 text-muted-foreground">
            <CheckCircle2 className="h-10 w-10 mx-auto mb-3 text-green-500" />
            <p className="text-sm font-medium">Aucune créance à relancer</p>
          </div>
        ) : (
          <div className="space-y-2">
            {creancesFiltrees.map(c => {
              const urg = urgence(c.jours_retard);
              const expanded = expandeId === c.vente_id;

              return (
                <div key={c.vente_id}
                  className="border border-border rounded-xl overflow-hidden bg-card">
                  {/* Ligne principale */}
                  <div className="flex items-center gap-3 px-4 py-3">
                    {/* Urgence badge */}
                    <span className={`shrink-0 text-xs font-medium px-2 py-1
                                      rounded-full border ${urg.couleur}`}>
                      {c.jours_retard}j
                    </span>

                    {/* Client + infos */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <p className="text-sm font-semibold truncate">{c.client_nom}</p>
                        {c.nb_relances === 0 && (
                          <Badge variant="outline" className="text-[10px] text-red-500
                                                              border-red-200 shrink-0">
                            Jamais relancé
                          </Badge>
                        )}
                        {c.nb_relances > 0 && (
                          <span className="text-[10px] text-muted-foreground shrink-0">
                            {c.nb_relances} relance{c.nb_relances > 1 ? "s" : ""}
                          </span>
                        )}
                      </div>
                      <div className="flex items-center gap-3 text-xs text-muted-foreground">
                        {c.facture_num && <span>{c.facture_num}</span>}
                        <span>{fmtDate(c.date_vente)}</span>
                        {c.telephone && (
                          <span className="flex items-center gap-1">
                            <Phone className="h-2.5 w-2.5" />{c.telephone}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Montant */}
                    <div className="text-right shrink-0">
                      <p className="text-sm font-bold text-orange-600">{fmt(c.reste)}</p>
                      {c.total_paye > 0 && (
                        <p className="text-xs text-muted-foreground">
                          payé {fmt(c.total_paye)}
                        </p>
                      )}
                    </div>

                    {/* Actions */}
                    <div className="flex items-center gap-1.5 shrink-0">
                      <Button size="sm"
                        className="h-7 text-xs gap-1.5 bg-green-600 hover:bg-green-700"
                        onClick={() => {
                          setCreanceActive(c);
                          setModalRelancer(true);
                        }}>
                        <MessageCircle className="h-3.5 w-3.5" />
                        Relancer
                      </Button>

                      {c.nb_relances > 0 && (
                        <button
                          onClick={() => {
                            setCreanceActive(c);
                            setModalHistorique(true);
                          }}
                          title="Voir l'historique"
                          className="h-7 w-7 flex items-center justify-center rounded
                                     text-muted-foreground hover:bg-muted transition-colors">
                          <History className="h-3.5 w-3.5" />
                        </button>
                      )}

                      <button
                        onClick={() => setExpandeId(expanded ? null : c.vente_id)}
                        className="h-7 w-7 flex items-center justify-center rounded
                                   text-muted-foreground hover:bg-muted transition-colors">
                        {expanded
                          ? <ChevronDown className="h-3.5 w-3.5" />
                          : <ChevronRight className="h-3.5 w-3.5" />
                        }
                      </button>
                    </div>
                  </div>

                  {/* Détail dépliable */}
                  {expanded && (
                    <div className="border-t border-border bg-muted/20 px-4 py-3
                                    text-xs text-muted-foreground space-y-1">
                      <div className="flex justify-between">
                        <span>Total facture</span>
                        <span className="font-medium text-foreground">{fmt(c.total)}</span>
                      </div>
                      {c.total_paye > 0 && (
                        <div className="flex justify-between">
                          <span>Acomptes reçus</span>
                          <span className="text-green-600">{fmt(c.total_paye)}</span>
                        </div>
                      )}
                      <div className="flex justify-between font-medium text-foreground
                                      border-t border-border pt-1 mt-1">
                        <span>Reste à recouvrir</span>
                        <span className="text-orange-600">{fmt(c.reste)}</span>
                      </div>
                      {c.derniere_relance && (
                        <p className="pt-1">
                          Dernière relance : {fmtDate(c.derniere_relance)}
                        </p>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Modals */}
      <ModalRelancer
        ouvert={modalRelancer}
        creance={creanceActive}
        societe={societe}
        onFermer={() => setModalRelancer(false)}
        onRelance={() => { setModalRelancer(false); charger(); }}
      />

      <ModalHistorique
        ouvert={modalHistorique}
        creance={creanceActive}
        onFermer={() => setModalHistorique(false)}
      />
    </div>
  );
}