// components/ModalPermissionsUtilisateur.tsx — les permissions d'UNE
// personne, par-dessus son rôle.
//
// Le rôle décide de l'essentiel ; cette fenêtre ajoute le sur-mesure
// prévu par D7 : autoriser ou refuser une permission à une personne,
// sans toucher au rôle. Trois états par permission :
//
//   - « Rôle »      (défaut, pas de ligne en base) ;
//   - « Autorisée » (accorde = 1, ajoute) ;
//   - « Refusée »   (accorde = 0, retire — le retrait l'emporte).
//
// Ce qui vient du rôle ne se coche pas ici : revenir en arrière
// demande de savoir ce que le rôle accordait, donc le troisième état
// « Rôle » efface le réglage personnel (accorde = null).

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Check, Loader2, Minus, Shield, Undo2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

interface Permission {
  code: string;
  libelle: string;
  groupe: string;
}

/** Le strict nécessaire : `OngletUtilisateurs` a déjà le reste. */
export interface UtilisateurPourPermissions {
  id: string;
  nom: string;
  role: string;
}

type Choix = "role" | "oui" | "non";

/** Regroupe les permissions par famille, dans l'ordre du catalogue. */
function parGroupe(catalogue: Permission[]): [string, Permission[]][] {
  const groupes: string[] = [];
  const par = new Map<string, Permission[]>();
  for (const p of catalogue) {
    if (!par.has(p.groupe)) {
      par.set(p.groupe, []);
      groupes.push(p.groupe);
    }
    par.get(p.groupe)!.push(p);
  }
  return groupes.map(g => [g, par.get(g)!]);
}

// =====================================================================
//  Le choix à trois états
// =====================================================================

function Segmente({ valeur, onChange }: {
  valeur: Choix;
  onChange: (c: Choix) => void;
}) {
  const options: { c: Choix; libelle: string; titre: string }[] = [
    { c: "role", libelle: "Rôle", titre: "Comme le rôle : ni ajout, ni retrait." },
    { c: "oui", libelle: "Autorisée", titre: "Ajoutée à cette personne." },
    { c: "non", libelle: "Refusée", titre: "Retirée à cette personne — le retrait l'emporte." },
  ];
  return (
    <div className="flex rounded-md border border-border overflow-hidden shrink-0">
      {options.map(o => (
        <button key={o.c} type="button" title={o.titre}
          onClick={() => onChange(o.c)}
          className={`px-2 py-1 text-xs transition-colors ${
            valeur === o.c
              ? o.c === "non"
                ? "bg-red-600 text-white"
                : "bg-primary text-primary-foreground"
              : "text-muted-foreground hover:bg-muted"
          }`}>
          {o.libelle}
        </button>
      ))}
    </div>
  );
}

// =====================================================================
//  La fenêtre
// =====================================================================

export function ModalPermissionsUtilisateur({
  ouvert, utilisateur, onFermer, onEnregistre,
}: {
  ouvert: boolean;
  utilisateur: UtilisateurPourPermissions | null;
  onFermer: () => void;
  onEnregistre: () => void;
}) {
  const [catalogue, setCatalogue] = useState<Permission[]>([]);
  const [effectives, setEffectives] = useState<Set<string>>(new Set());
  const [initial, setInitial] = useState<Map<string, Choix>>(new Map());
  const [courant, setCourant] = useState<Map<string, Choix>>(new Map());
  const [chargement, setChargement] = useState(true);
  const [enregistrement, setEnregistrement] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    if (!ouvert || !utilisateur) return;
    setChargement(true);
    setErreur(null);
    (async () => {
      try {
        const [c, d] = await Promise.all([
          invoke<Permission[]>("lire_catalogue_permissions"),
          invoke<{
            role: string;
            effectives: string[];
            reglages: { permission: string; accorde: boolean }[];
          }>("lire_permissions_utilisateur", {
            utilisateurId: utilisateur.id,
          }),
        ]);
        setCatalogue(c);
        setEffectives(new Set(d.effectives));
        const reglages = new Map<string, Choix>(
          d.reglages.map(r => [r.permission, r.accorde ? "oui" : "non"]),
        );
        setInitial(reglages);
        setCourant(new Map(reglages));
      } catch (e) {
        setErreur(String(e));
      } finally {
        setChargement(false);
      }
    })();
  }, [ouvert, utilisateur]);

  const nbReglages = [...courant.values()].filter(c => c !== "role").length;

  async function enregistrer() {
    if (!utilisateur) return;
    setEnregistrement(true);
    try {
      // Une commande par permission changée : le noyau refuse ce qui
      // n'est pas au catalogue, et le retrait personnel se pose en
      // ligne (accorde = 0), pas en absence de ligne.
      for (const p of catalogue) {
        const avant = initial.get(p.code) ?? "role";
        const apres = courant.get(p.code) ?? "role";
        if (avant === apres) continue;
        await invoke("definir_permission_utilisateur", {
          utilisateurId: utilisateur.id,
          permission: p.code,
          accorde: apres === "role" ? null : apres === "oui",
        });
      }
      onEnregistre();
    } catch (e) {
      await message(String(e), { title: "Erreur", kind: "error" });
    } finally {
      setEnregistrement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-2xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="h-4 w-4" />
            {utilisateur
              ? `Permissions de ${utilisateur.nom}`
              : "Permissions"}
          </DialogTitle>
        </DialogHeader>

        {chargement ? (
          <div className="flex justify-center py-8">
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          </div>
        ) : erreur ? (
          <p className="text-sm text-red-600 py-4">{erreur}</p>
        ) : (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <p className="text-sm text-muted-foreground">
                Rôle « {utilisateur?.role} » · {nbReglages} réglage
                {nbReglages > 1 ? "s" : ""} personnel{nbReglages > 1 ? "s" : ""}
              </p>
              <Button variant="outline" size="sm"
                disabled={nbReglages === 0 || enregistrement}
                onClick={() => setCourant(new Map())}
                title="Chaque permission redevient ce que le rôle en dit">
                <Undo2 className="h-3.5 w-3.5 mr-1" /> Tout remettre au rôle
              </Button>
            </div>

            {parGroupe(catalogue).map(([groupe, permissions]) => (
              <div key={groupe} className="border border-border rounded-lg p-3">
                <p className="text-xs font-semibold uppercase tracking-wide
                              text-muted-foreground mb-1">
                  {groupe}
                </p>
                <div className="space-y-0.5">
                  {permissions.map(p => {
                    const choix = courant.get(p.code) ?? "role";
                    const effective = effectives.has(p.code);
                    return (
                      <div key={p.code}
                        className="flex items-center justify-between gap-2 py-1">
                        <div className="flex items-start gap-2 text-sm min-w-0">
                          {effective
                            ? <Check className="h-4 w-4 mt-0.5 text-emerald-600 shrink-0" />
                            : <Minus className="h-4 w-4 mt-0.5 text-muted-foreground shrink-0" />}
                          <span className="min-w-0">
                            {p.libelle}
                            <span className="text-muted-foreground text-xs ml-2">
                              {p.code}
                            </span>
                          </span>
                        </div>
                        <Segmente valeur={choix}
                          onChange={c => setCourant(prev => {
                            const suite = new Map(prev);
                            if (c === "role") suite.delete(p.code);
                            else suite.set(p.code, c);
                            return suite;
                          })} />
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}

            <p className="text-xs text-muted-foreground">
              La coche montre ce que la personne peut réellement. Le
              rôle décide ; ici on ajuste pour une personne. « Refusée »
              l'emporte toujours, même si le rôle l'accorde.
            </p>

            <div className="flex gap-2 pt-2">
              <Button variant="outline" onClick={onFermer}
                disabled={enregistrement} className="flex-1">Annuler</Button>
              <Button onClick={enregistrer} disabled={enregistrement}
                className="flex-1">
                {enregistrement
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : "Enregistrer"}
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
