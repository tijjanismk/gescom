// pages/AssistantDossier.tsx
//
// D43 — l'assistant de création du dossier, au premier démarrage, comme
// dans Ciel. Une base neuve s'ouvre avec « admin / admin123 » ; tant que
// l'assistant n'est pas fini, le serveur ne sert que lui
// (ASSISTANT_A_FAIRE) : personne ne vend dans un dossier sans nom, avec
// un mot de passe d'usine et des dates posées par défaut.
//
// L'écran guide, le serveur juge (D27) : `terminer_assistant` reprend
// chaque champ et écrit tout en une transaction. Le logo part par sa
// propre commande, comme dans Paramètres. La reprise des données
// (articles, fournisseurs) vient APRÈS la création : le dossier a alors
// son magasin et sa TVA, que l'import utilise.

import { useEffect, useMemo, useState } from "react";
import { appeler as invoke, renommerDossierCourant } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft, ArrowRight, Building2, CalendarRange, Check, CheckCircle2, Image as ImageIcon,
  Loader2, LogOut, Plus, Settings2, Sparkles, Trash2, Upload, UserRound, Users, Truck, Package,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { envoyerImage } from "@/components/ParametresSociete";
import { ImportCsvAvecApercu } from "@/components/ImportCsvAvecApercu";
import { ImportExportFournisseurs } from "@/components/ImportExportFournisseurs";
import { fmt } from "@/lib/remise";

/** Ce que rend `lire_assistant`. */
interface Proposition {
  a_faire: boolean;
  societe: { nom?: string; adresse?: string; telephone?: string; telephone2?: string; email?: string;
             nif?: string; rccm?: string; site_web?: string; pied_facture?: string } | null;
  date_debut: string | null;
  date_fin: string | null;
  magasin: string | null;
  tva_pct: number;
  fond_caisse: number;
  patron_nom: string | null;
  patron_pseudo: string | null;
  annee: string;
  aujourd_hui: string;
  tva_normale_pct: number;
  numero_depart_max: number;
  duree_max_ans: number;
}

interface Vendeur { nom: string; pseudo: string; mot_de_passe: string }

const NOMS_USINE = ["ma boutique", "ma société", "ma societe"];

const ETAPES = [
  { titre: "Bienvenue", icone: Sparkles },
  { titre: "Vous", icone: UserRound },
  { titre: "La société", icone: Building2 },
  { titre: "Les dates de travail", icone: CalendarRange },
  { titre: "Les réglages", icone: Settings2 },
  { titre: "Les vendeurs", icone: Users },
  { titre: "Vérifier et créer", icone: Check },
  { titre: "Reprendre vos données", icone: Package },
];

const MOIS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août",
              "septembre", "octobre", "novembre", "décembre"];

function enLettres(iso: string): string {
  const [a, m, j] = iso.split("-").map(Number);
  if (!a || !m || !j) return iso;
  return `${j === 1 ? "1er" : j} ${MOIS[m - 1]} ${a}`;
}

/** Nombre de mois couverts, arrondi (pour la phrase en clair). */
function duree(debut: string, fin: string): string {
  const d = new Date(debut), f = new Date(fin);
  if (isNaN(d.getTime()) || isNaN(f.getTime()) || f < d) return "";
  const mois = Math.round((f.getTime() - d.getTime()) / (1000 * 60 * 60 * 24 * 30.44));
  if (mois >= 12 && mois % 12 === 0) return `${mois / 12} an${mois > 12 ? "s" : ""}`;
  return `${mois} mois`;
}

function finDans12Mois(debut: string): string {
  const d = new Date(debut);
  if (isNaN(d.getTime())) return "";
  const f = new Date(d.getFullYear() + 1, d.getMonth(), d.getDate() - 1);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${f.getFullYear()}-${p(f.getMonth() + 1)}-${p(f.getDate())}`;
}

interface Props {
  utilisateur: { id: string; nom: string };
  /** L'assistant fini : le nom du patron a pu changer. */
  onTermine: (patronNom: string) => void;
  onDeconnecter: () => void;
}

export function AssistantDossier({ utilisateur, onTermine, onDeconnecter }: Props) {
  // Seul qui peut gérer les dossiers fait l'assistant (le patron).
  if (!peut("dossiers:gerer")) {
    return (
      <Cadre>
        <div className="max-w-md mx-auto text-center space-y-4 py-16">
          <Building2 className="h-10 w-10 mx-auto text-muted-foreground" />
          <h1 className="text-xl font-semibold">Le dossier n'est pas encore créé</h1>
          <p className="text-sm text-muted-foreground">
            Bonjour {utilisateur.nom}. Le patron crée le dossier de la boutique à sa
            première connexion. Vous pourrez travailler dès qu'il l'aura fait.
          </p>
          <Button variant="outline" onClick={onDeconnecter}>
            <LogOut className="h-4 w-4 mr-2" /> Se déconnecter
          </Button>
        </div>
      </Cadre>
    );
  }
  return <Assistant utilisateur={utilisateur} onTermine={onTermine} onDeconnecter={onDeconnecter} />;
}

function Assistant({ onTermine, onDeconnecter }: Props) {
  const [prop, setProp] = useState<Proposition | null>(null);
  const [erreurChargement, setErreurChargement] = useState("");
  const [etape, setEtape] = useState(0);
  const [erreur, setErreur] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [cree, setCree] = useState(false);

  // 1. Vous
  const [patronNom, setPatronNom] = useState("");
  const [pseudo, setPseudo] = useState("");
  const [mdp, setMdp] = useState("");
  const [mdp2, setMdp2] = useState("");
  // 2. La société
  const [soc, setSoc] = useState({ nom: "", adresse: "", telephone: "", telephone2: "", email: "",
                                   nif: "", rccm: "", site_web: "", pied_facture: "Merci de votre confiance" });
  const [logo, setLogo] = useState<string | null>(null);
  const [envoiLogo, setEnvoiLogo] = useState(false);
  // 3. Les dates
  const [debut, setDebut] = useState("");
  const [fin, setFin] = useState("");
  // 4. Les réglages
  const [tva, setTva] = useState(0);
  const [avecNumero, setAvecNumero] = useState(false);
  const [numero, setNumero] = useState("");
  const [magasin, setMagasin] = useState("");
  const [fond, setFond] = useState("");
  // 5. Les vendeurs
  const [vendeurs, setVendeurs] = useState<Vendeur[]>([]);

  useEffect(() => {
    Promise.all([
      invoke<Proposition>("lire_assistant"),
      invoke<string | null>("lire_logo_base64").catch(() => null),
    ]).then(([p, l]) => {
      setProp(p);
      setLogo(l);
      setPatronNom(p.patron_nom && p.patron_nom !== "Patron" ? p.patron_nom : "");
      setPseudo(p.patron_pseudo ?? "admin");
      const s = p.societe ?? {};
      const nom = s.nom && !NOMS_USINE.includes(s.nom.trim().toLowerCase()) ? s.nom : "";
      setSoc(v => ({
        ...v, nom, adresse: s.adresse ?? "", telephone: s.telephone ?? "", telephone2: s.telephone2 ?? "",
        email: s.email ?? "", nif: s.nif ?? "", rccm: s.rccm ?? "", site_web: s.site_web ?? "",
        pied_facture: s.pied_facture ?? v.pied_facture,
      }));
      setDebut(p.date_debut ?? `${p.annee}-01-01`);
      setFin(p.date_fin ?? `${p.annee}-12-31`);
      setTva(p.tva_pct);
      setMagasin(p.magasin ?? "Magasin principal");
      setFond(p.fond_caisse > 0 ? String(p.fond_caisse) : "");
    }).catch(e => setErreurChargement(String(e)));
  }, []);

  // Ce qui manque pour passer à l'étape suivante — l'écran aide, le
  // serveur jugera tout de nouveau.
  const manque = useMemo((): string => {
    if (!prop) return "";
    switch (etape) {
      case 1:
        if (!patronNom.trim()) return "Votre nom.";
        if (pseudo.trim().length < 3) return "Un identifiant de 3 caractères au moins.";
        if (mdp.length < 6) return "Un mot de passe de 6 caractères au moins.";
        if (mdp === "admin123") return "Le mot de passe d'usine ne se garde pas.";
        if (mdp !== mdp2) return "Les deux mots de passe diffèrent.";
        return "";
      case 2:
        return soc.nom.trim() ? "" : "La raison sociale.";
      case 3:
        if (!debut || !fin) return "Le début et la fin.";
        if (fin < debut) return "La fin précède le début.";
        if (prop.aujourd_hui < debut) return "Les dates doivent comprendre aujourd'hui : sinon aucune vente ne passerait.";
        if (prop.aujourd_hui > fin) return "Les dates doivent comprendre aujourd'hui : sinon aucune vente ne passerait.";
        return "";
      case 4:
        if (!magasin.trim()) return "Le nom du magasin principal.";
        if (avecNumero) {
          const n = Number(numero);
          if (!Number.isInteger(n) || n < 1 || n > prop.numero_depart_max) return `Un premier numéro de 1 à ${prop.numero_depart_max}.`;
        }
        return "";
      case 5:
        for (const v of vendeurs) {
          if (!v.nom.trim() || v.pseudo.trim().length < 3 || v.mot_de_passe.length < 6)
            return "Chaque vendeur : un nom, un identifiant (3 caractères), un mot de passe (6 caractères).";
        }
        return "";
      default:
        return "";
    }
  }, [prop, etape, patronNom, pseudo, mdp, mdp2, soc.nom, debut, fin, magasin, avecNumero, numero, vendeurs]);

  async function choisirLogo() {
    setEnvoiLogo(true);
    try {
      const fichier = await open({ multiple: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "webp"] }] });
      if (!fichier || typeof fichier !== "string") return;
      await envoyerImage("sauvegarder_logo", fichier);
      setLogo(await invoke<string | null>("lire_logo_base64"));
    } catch (e) {
      setErreur(`Logo : ${e}`);
    } finally {
      setEnvoiLogo(false);
    }
  }

  async function creer() {
    setEnCours(true);
    setErreur("");
    try {
      const texte = (v: string) => v.trim() || null;
      await invoke("terminer_assistant", {
        saisie: {
          patron_nom: patronNom, patron_pseudo: pseudo, mot_de_passe: mdp,
          societe: soc.nom, adresse: texte(soc.adresse), telephone: texte(soc.telephone),
          telephone2: texte(soc.telephone2), email: texte(soc.email), nif: texte(soc.nif),
          rccm: texte(soc.rccm), site_web: texte(soc.site_web), pied_facture: texte(soc.pied_facture),
          date_debut: debut, date_fin: fin,
          tva_pct: tva,
          numero_depart: avecNumero ? Number(numero) : null,
          magasin, fond_caisse: parseMontant(fond) || 0,
          vendeurs,
        },
      });
      // La barre de gauche montre le nom du dossier recu a la connexion
      // (« Ma boutique ») : elle suit la raison sociale.
      renommerDossierCourant(soc.nom.trim());
      setCree(true);
      setEtape(7);
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  if (erreurChargement) {
    return (
      <Cadre>
        <div className="max-w-md mx-auto text-center space-y-4 py-16">
          <p className="text-sm text-destructive">{erreurChargement}</p>
          <Button variant="outline" onClick={onDeconnecter}>Se déconnecter</Button>
        </div>
      </Cadre>
    );
  }
  if (!prop) {
    return <Cadre><div className="flex justify-center py-24"><Loader2 className="h-6 w-6 animate-spin text-muted-foreground" /></div></Cadre>;
  }

  const suivant = () => { setErreur(""); setEtape(e => e + 1); };
  const precedent = () => { setErreur(""); setEtape(e => Math.max(0, e - 1)); };

  return (
    <Cadre>
      <div className="flex gap-8 max-w-5xl mx-auto w-full">
        {/* Les étapes */}
        <ol className="hidden md:block w-56 shrink-0 space-y-1 pt-2">
          {ETAPES.map((e, i) => {
            const Icone = e.icone;
            const fait = i < etape || (cree && i <= 6);
            return (
              <li key={i} className={`flex items-center gap-2 rounded-md px-3 py-2 text-sm
                ${i === etape ? "bg-primary/10 text-primary font-medium" : fait ? "text-foreground" : "text-muted-foreground"}`}>
                {fait && i !== etape ? <CheckCircle2 className="h-4 w-4 text-green-600" /> : <Icone className="h-4 w-4" />}
                {e.titre}
              </li>
            );
          })}
        </ol>

        {/* L'étape */}
        <div className="flex-1 min-w-0 space-y-6">
          <div>
            <p className="text-xs text-muted-foreground">Étape {etape + 1} sur {ETAPES.length}</p>
            <h1 className="text-2xl font-semibold">{ETAPES[etape].titre}</h1>
          </div>

          {etape === 0 && (
            <div className="space-y-4 text-sm">
              <p>
                Bonjour. Avant la première vente, Gescom crée le <strong>dossier de votre boutique</strong> :
                son nom sur les factures, ses dates de travail, votre mot de passe et quelques réglages.
                Cela prend cinq minutes ; tout se change ensuite dans Paramètres.
              </p>
              <ul className="space-y-1.5 text-muted-foreground">
                <li>· <strong className="text-foreground">Vous</strong> — votre nom, votre identifiant, un mot de passe à vous (celui d'usine est public).</li>
                <li>· <strong className="text-foreground">La société</strong> — raison sociale, adresse, téléphones, NIF, RCCM, logo.</li>
                <li>· <strong className="text-foreground">Les dates de travail</strong> — le premier exercice, comme dans Ciel.</li>
                <li>· <strong className="text-foreground">Les réglages</strong> — TVA, premier numéro de facture, magasin, fond de caisse.</li>
                <li>· <strong className="text-foreground">Les vendeurs</strong> — leurs comptes, si vous le voulez.</li>
                <li>· <strong className="text-foreground">Vos données</strong> — vos articles et fournisseurs depuis Excel, si vous en avez.</li>
              </ul>
              <p className="text-muted-foreground">
                Rien n'est écrit avant « Créer mon dossier », à l'avant-dernière étape. Les montants sont en
                francs CFA, sans centimes.
              </p>
            </div>
          )}

          {etape === 1 && (
            <div className="space-y-4 max-w-md">
              <Champ id="patron-nom" libelle="Votre nom *" valeur={patronNom} onChange={setPatronNom} placeholder="Moussa Traoré" autoFocus />
              <Champ id="patron-pseudo" libelle="Votre identifiant de connexion *" valeur={pseudo} onChange={setPseudo}
                aide="Lettres sans accent, chiffres, point, tiret. À taper tel quel à chaque connexion." />
              <Champ id="patron-mdp" libelle="Nouveau mot de passe *" valeur={mdp} onChange={setMdp} type="password"
                aide="6 caractères au moins. « admin123 » est écrit dans le manuel : il ne se garde pas." />
              <Champ id="patron-mdp2" libelle="Le même, une seconde fois *" valeur={mdp2} onChange={setMdp2} type="password" />
            </div>
          )}

          {etape === 2 && (
            <div className="space-y-4">
              <p className="text-sm text-muted-foreground">Ce qui figure en tête de chaque facture et de chaque reçu.</p>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div className="sm:col-span-2">
                  <Champ id="soc-nom" libelle="Raison sociale *" valeur={soc.nom} onChange={v => setSoc({ ...soc, nom: v })}
                    placeholder="Quincaillerie Traoré" autoFocus aide="Elle devient aussi le nom du dossier." />
                </div>
                <div className="sm:col-span-2">
                  <Champ id="soc-adresse" libelle="Adresse" valeur={soc.adresse} onChange={v => setSoc({ ...soc, adresse: v })} placeholder="Marché de Médine, Bamako" />
                </div>
                <Champ id="soc-tel" libelle="Téléphone" valeur={soc.telephone} onChange={v => setSoc({ ...soc, telephone: v })} placeholder="76 00 00 00" />
                <Champ id="soc-tel2" libelle="Second téléphone" valeur={soc.telephone2} onChange={v => setSoc({ ...soc, telephone2: v })} />
                <Champ id="soc-nif" libelle="NIF" valeur={soc.nif} onChange={v => setSoc({ ...soc, nif: v })} />
                <Champ id="soc-rccm" libelle="RCCM" valeur={soc.rccm} onChange={v => setSoc({ ...soc, rccm: v })} placeholder="MA.BKO.2026.B.1234" />
                <Champ id="soc-email" libelle="Email" valeur={soc.email} onChange={v => setSoc({ ...soc, email: v })} />
                <Champ id="soc-site" libelle="Site web" valeur={soc.site_web} onChange={v => setSoc({ ...soc, site_web: v })} />
                <div className="sm:col-span-2">
                  <Champ id="soc-pied" libelle="Pied de facture" valeur={soc.pied_facture} onChange={v => setSoc({ ...soc, pied_facture: v })} />
                </div>
              </div>
              <div>
                <Label className="mb-2 block">Logo</Label>
                <div className="flex items-center gap-3">
                  {logo
                    ? <img src={logo} alt="Logo" className="h-16 w-auto max-w-[160px] object-contain border border-border rounded-md p-1" />
                    : <div className="h-16 w-16 rounded-md border border-dashed border-border flex items-center justify-center text-muted-foreground"><ImageIcon className="h-5 w-5" /></div>}
                  <Button variant="outline" size="sm" onClick={choisirLogo} disabled={envoiLogo}>
                    {envoiLogo ? <Loader2 className="h-4 w-4 animate-spin" /> : <><Upload className="h-4 w-4 mr-2" />{logo ? "Changer" : "Choisir une image"}</>}
                  </Button>
                  <span className="text-xs text-muted-foreground">Facultatif. PNG ou JPG.</span>
                </div>
              </div>
            </div>
          )}

          {etape === 3 && (
            <div className="space-y-4 max-w-xl">
              <p className="text-sm text-muted-foreground">
                Le premier exercice : la période où les ventes, les achats et la caisse s'écrivent. Comme dans Ciel,
                il peut commencer n'importe quel jour et durer jusqu'à {prop.duree_max_ans} ans. Il doit comprendre
                aujourd'hui. Avant la fin, Gescom prévient ; on prolonge ou on ouvre le suivant.
              </p>
              <div className="flex flex-wrap gap-2">
                <Button size="sm" variant="outline" onClick={() => { setDebut(`${prop.annee}-01-01`); setFin(`${prop.annee}-12-31`); }}>
                  Année {prop.annee}
                </Button>
                <Button size="sm" variant="outline" onClick={() => {
                  const d = `${prop.aujourd_hui.slice(0, 8)}01`; setDebut(d); setFin(finDans12Mois(d));
                }}>
                  12 mois à partir de ce mois
                </Button>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <Label htmlFor="debut">Début *</Label>
                  <Input id="debut" type="date" value={debut} onChange={e => setDebut(e.target.value)} className="mt-1" />
                </div>
                <div>
                  <Label htmlFor="fin">Fin *</Label>
                  <Input id="fin" type="date" value={fin} onChange={e => setFin(e.target.value)} className="mt-1" />
                </div>
              </div>
              {debut && fin && fin >= debut && (
                <p className="text-sm">
                  Du <strong>{enLettres(debut)}</strong> au <strong>{enLettres(fin)}</strong>
                  {duree(debut, fin) && <> — {duree(debut, fin)}</>}.
                </p>
              )}
              <p className="text-xs text-muted-foreground">
                Les numéros de pièce suivent l'année civile (FAC-{prop.annee}-…) : ils repartent à 1 chaque 1er janvier, quel que soit l'exercice.
              </p>
            </div>
          )}

          {etape === 4 && (
            <div className="space-y-6 max-w-xl">
              <div className="space-y-2">
                <Label>La boutique facture-t-elle la TVA ?</Label>
                <div className="flex gap-2">
                  <Button type="button" variant={tva === 0 ? "default" : "outline"} onClick={() => setTva(0)}>Non</Button>
                  <Button type="button" variant={tva === prop.tva_normale_pct ? "default" : "outline"} onClick={() => setTva(prop.tva_normale_pct)}>
                    Oui, {prop.tva_normale_pct} %
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  La plupart des boutiques sont à l'impôt synthétique : pas de TVA. Ce taux est celui des nouveaux articles ;
                  chacun se règle ensuite à part.
                </p>
              </div>

              <div className="space-y-2">
                <label className="flex items-center gap-2 text-sm cursor-pointer">
                  <input type="checkbox" checked={avecNumero} onChange={e => setAvecNumero(e.target.checked)} className="w-4 h-4 accent-primary" />
                  Continuer une numérotation de factures déjà commencée
                </label>
                {avecNumero && (
                  <div className="max-w-[220px]">
                    <Label htmlFor="numero">Première facture de {prop.annee} *</Label>
                    <Input id="numero" inputMode="numeric" value={numero} onChange={e => setNumero(e.target.value.replace(/\D/g, ""))}
                      placeholder="1251" className="mt-1" />
                    {Number(numero) > 0 && (
                      <p className="text-xs text-muted-foreground mt-1 font-mono">
                        FAC-{prop.annee}-{String(Number(numero)).padStart(5, "0")}
                      </p>
                    )}
                  </div>
                )}
                <p className="text-xs text-muted-foreground">
                  Pour qui quitte un carnet ou un autre logiciel. Sinon, la première facture porte le numéro 1.
                  Ce réglage ne se fait qu'ici, avant toute facture.
                </p>
              </div>

              <Champ id="magasin" libelle="Nom du magasin principal *" valeur={magasin} onChange={setMagasin}
                aide="Il figure sur les bons et les transferts. D'autres magasins se créent ensuite dans Paramètres." />

              <div>
                <Label htmlFor="fond">Fond de caisse habituel (F)</Label>
                <MoneyInput id="fond" value={fond} onChange={setFond} placeholder="0" className="mt-1 max-w-[220px]" />
                <p className="text-xs text-muted-foreground mt-1">
                  La monnaie laissée dans le tiroir chaque matin. Proposée à l'ouverture de la caisse, toujours modifiable.
                </p>
              </div>
            </div>
          )}

          {etape === 5 && (
            <div className="space-y-4 max-w-2xl">
              <p className="text-sm text-muted-foreground">
                Facultatif. Chaque vendeur a son compte : ses ventes et sa caisse portent son nom. Le mot de passe
                donné ici est provisoire ; il le change à sa première connexion. Les droits se règlent ensuite dans
                Paramètres → Utilisateurs.
              </p>
              {vendeurs.map((v, i) => (
                <div key={i} className="grid grid-cols-[1fr_1fr_1fr_auto] gap-2 items-end">
                  <Champ id={`v-nom-${i}`} libelle="Nom" valeur={v.nom}
                    onChange={x => setVendeurs(vendeurs.map((w, j) => j === i ? { ...w, nom: x } : w))} />
                  <Champ id={`v-pseudo-${i}`} libelle="Identifiant" valeur={v.pseudo}
                    onChange={x => setVendeurs(vendeurs.map((w, j) => j === i ? { ...w, pseudo: x } : w))} />
                  <Champ id={`v-mdp-${i}`} libelle="Mot de passe provisoire" valeur={v.mot_de_passe}
                    onChange={x => setVendeurs(vendeurs.map((w, j) => j === i ? { ...w, mot_de_passe: x } : w))} />
                  <Button variant="ghost" size="icon" aria-label="Retirer ce vendeur"
                    onClick={() => setVendeurs(vendeurs.filter((_, j) => j !== i))}>
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}
              <Button variant="outline" size="sm" onClick={() => setVendeurs([...vendeurs, { nom: "", pseudo: "", mot_de_passe: "" }])}>
                <Plus className="h-4 w-4 mr-1" /> Ajouter un vendeur
              </Button>
            </div>
          )}

          {etape === 6 && (
            <div className="space-y-4 max-w-2xl">
              <Card>
                <CardContent className="pt-4 text-sm space-y-2">
                  <Ligne titre="Vous" valeur={`${patronNom} — identifiant « ${pseudo} », nouveau mot de passe`} />
                  <Ligne titre="Société" valeur={[soc.nom, soc.adresse, soc.telephone].filter(Boolean).join(" · ")} />
                  {(soc.nif || soc.rccm) && <Ligne titre="NIF / RCCM" valeur={[soc.nif, soc.rccm].filter(Boolean).join(" · ")} />}
                  <Ligne titre="Logo" valeur={logo ? "choisi" : "aucun (se choisit plus tard dans Paramètres → Société)"} />
                  <Ligne titre="Dates de travail" valeur={`du ${enLettres(debut)} au ${enLettres(fin)}`} />
                  <Ligne titre="TVA" valeur={tva === 0 ? "pas de TVA" : `${tva} % sur les nouveaux articles`} />
                  <Ligne titre="Première facture" valeur={avecNumero ? `FAC-${prop.annee}-${String(Number(numero)).padStart(5, "0")}` : `FAC-${prop.annee}-00001`} />
                  <Ligne titre="Magasin principal" valeur={magasin} />
                  <Ligne titre="Fond de caisse" valeur={parseMontant(fond) ? `${fmt(parseMontant(fond))} F` : "aucun"} />
                  <Ligne titre="Vendeurs" valeur={vendeurs.length ? vendeurs.map(v => `${v.nom} (${v.pseudo})`).join(", ") : "aucun pour l'instant"} />
                </CardContent>
              </Card>
              <p className="text-sm text-muted-foreground">
                Après « Créer mon dossier », vous vous connecterez avec l'identifiant <strong>{pseudo}</strong> et votre
                nouveau mot de passe. Tout le reste se change dans Paramètres.
              </p>
            </div>
          )}

          {etape === 7 && (
            <div className="space-y-6">
              <div className="flex items-center gap-2 rounded-md border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-900">
                <CheckCircle2 className="h-4 w-4" />
                Le dossier « {soc.nom.trim()} » est créé. Vous pouvez reprendre vos articles et vos fournisseurs depuis Excel
                — ou commencer tout de suite et le faire plus tard.
              </div>
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                <Card>
                  <CardContent className="pt-4 space-y-3">
                    <h3 className="text-sm font-semibold flex items-center gap-2"><Package className="h-4 w-4" /> Vos articles</h3>
                    <p className="text-xs text-muted-foreground">
                      Un fichier CSV (Excel → Enregistrer sous → CSV point-virgule) : Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock.
                      Le modèle complet est dans Paramètres → Import / export.
                    </p>
                    <ImportCsvAvecApercu
                      commande="importer_articles_csv"
                      quoi="articles"
                      option={{ libelle: "Mettre à jour les articles déjà présents", cle: "mettreAJour", parDefaut: true }}
                      libelleRepris="Mis à jour"
                    />
                  </CardContent>
                </Card>
                <Card>
                  <CardContent className="pt-4 space-y-3">
                    <h3 className="text-sm font-semibold flex items-center gap-2"><Truck className="h-4 w-4" /> Vos fournisseurs</h3>
                    <ImportExportFournisseurs />
                  </CardContent>
                </Card>
              </div>
            </div>
          )}

          {erreur && <p className="text-sm text-destructive whitespace-pre-line">{erreur}</p>}
          {manque && etape > 0 && etape < 6 && <p className="text-xs text-muted-foreground">À remplir : {manque}</p>}

          <div className="flex items-center gap-2 pt-2 border-t border-border">
            {etape > 0 && etape < 7 && (
              <Button variant="outline" onClick={precedent} disabled={enCours}>
                <ArrowLeft className="h-4 w-4 mr-1" /> Précédent
              </Button>
            )}
            <div className="flex-1" />
            {etape < 6 && (
              <Button onClick={suivant} disabled={!!manque}>
                {etape === 0 ? "Commencer" : "Suivant"} <ArrowRight className="h-4 w-4 ml-1" />
              </Button>
            )}
            {etape === 6 && (
              <Button onClick={creer} disabled={enCours} size="lg">
                {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : <><Check className="h-4 w-4 mr-1" /> Créer mon dossier</>}
              </Button>
            )}
            {etape === 7 && (
              <Button onClick={() => onTermine(patronNom.trim())} size="lg">
                Commencer à travailler <ArrowRight className="h-4 w-4 ml-1" />
              </Button>
            )}
          </div>
        </div>
      </div>
    </Cadre>
  );
}

function Cadre({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-background flex flex-col">
      <header className="flex items-center gap-2 px-6 py-4 border-b border-border">
        <Building2 className="h-5 w-5 text-primary" />
        <span className="font-semibold">Gescom</span>
        <span className="text-sm text-muted-foreground">— création du dossier</span>
      </header>
      <main className="flex-1 overflow-auto p-6">{children}</main>
    </div>
  );
}

function Champ({ id, libelle, valeur, onChange, type = "text", placeholder, aide, autoFocus }: {
  id: string; libelle: string; valeur: string; onChange: (v: string) => void;
  type?: string; placeholder?: string; aide?: string; autoFocus?: boolean;
}) {
  return (
    <div>
      <Label htmlFor={id}>{libelle}</Label>
      <Input id={id} type={type} value={valeur} onChange={e => onChange(e.target.value)}
        placeholder={placeholder} className="mt-1" autoFocus={autoFocus}
        autoComplete={type === "password" ? "new-password" : "off"} />
      {aide && <p className="text-xs text-muted-foreground mt-1">{aide}</p>}
    </div>
  );
}

function Ligne({ titre, valeur }: { titre: string; valeur: string }) {
  return (
    <div className="grid grid-cols-[160px_1fr] gap-2">
      <span className="text-muted-foreground">{titre}</span>
      <span>{valeur}</span>
    </div>
  );
}
