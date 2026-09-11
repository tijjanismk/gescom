// components/OngletRoles.tsx — les rôles et ce qu'ils permettent.
//
// Le commerçant décide qui fait quoi. Jusqu'ici c'était figé dans le
// code : patron ou employé, rien entre les deux. Il peut désormais
// partir des rôles livrés — caissier, magasinier, comptable — les
// ajuster, ou en fabriquer d'autres.
//
// Deux règles de présentation, et elles comptent plus qu'on ne croit :
//
//   1. Un rôle à accès total montre ses cases COCHÉES ET GRISÉES.
//      Afficher une liste vide laisserait croire qu'il ne peut rien ;
//      afficher des cases modifiables laisserait croire qu'on peut lui
//      retirer quelque chose.
//   2. Le rôle protégé ne s'ouvre pas du tout. C'est le compte de
//      secours : lui laisser un bouton « Enregistrer » qui échoue est
//      pire que ne pas l'offrir.

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message, confirm } from "@tauri-apps/plugin-dialog";
import {
  Loader2, Plus, Shield, ShieldCheck, Lock, Trash2, Users,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

interface Permission {
  code: string;
  libelle: string;
  groupe: string;
}

interface Role {
  id: string;
  nom: string;
  permissions: string[];
  acces_total: boolean;
  protege: boolean;
  description: string;
  nb_utilisateurs: number;
}

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
//  Modal : créer ou modifier un rôle
// =====================================================================

function ModalRole({
  ouvert, role, catalogue, onFermer, onEnregistre,
}: {
  ouvert: boolean;
  /** null = création. */
  role: Role | null;
  catalogue: Permission[];
  onFermer: () => void;
  onEnregistre: () => void;
}) {
  const [nom, setNom] = useState("");
  const [description, setDescription] = useState("");
  const [cochees, setCochees] = useState<Set<string>>(new Set());
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (!ouvert) return;
    setNom(role?.nom ?? "");
    setDescription(role?.description ?? "");
    setCochees(new Set(role?.permissions ?? []));
  }, [ouvert, role]);

  function basculer(code: string) {
    setCochees(prev => {
      const suite = new Set(prev);
      if (suite.has(code)) suite.delete(code);
      else suite.add(code);
      return suite;
    });
  }

  function basculerGroupe(permissions: Permission[]) {
    const tout = permissions.every(p => cochees.has(p.code));
    setCochees(prev => {
      const suite = new Set(prev);
      for (const p of permissions) {
        if (tout) suite.delete(p.code);
        else suite.add(p.code);
      }
      return suite;
    });
  }

  async function enregistrer() {
    setChargement(true);
    try {
      const permissions = [...cochees];
      if (role) {
        await invoke("modifier_role", {
          roleId: role.id, description: description || null, permissions,
        });
      } else {
        await invoke("creer_role", {
          nom, description: description || null, permissions,
        });
      }
      onEnregistre();
    } catch (e) {
      await message(String(e), { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-2xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="h-4 w-4" />
            {role ? `Rôle « ${role.nom} »` : "Nouveau rôle"}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {!role && (
            <div>
              <Label>Nom *</Label>
              <Input value={nom} onChange={e => setNom(e.target.value)}
                placeholder="livreur" className="mt-1" />
              <p className="text-xs text-muted-foreground mt-1">
                En minuscules, sans espace. C'est ce qui s'affichera à
                côté du nom de la personne.
              </p>
            </div>
          )}

          <div>
            <Label>À quoi sert ce rôle</Label>
            <Input value={description} onChange={e => setDescription(e.target.value)}
              placeholder="Livre la marchandise, rien d'autre."
              className="mt-1" />
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <Label>Ce que ce rôle permet</Label>
              <span className="text-xs text-muted-foreground">
                {cochees.size} / {catalogue.length}
              </span>
            </div>

            {parGroupe(catalogue).map(([groupe, permissions]) => (
              <div key={groupe} className="border border-border rounded-lg p-3">
                <button type="button"
                  onClick={() => basculerGroupe(permissions)}
                  className="text-xs font-semibold uppercase tracking-wide
                             text-muted-foreground hover:text-foreground
                             transition-colors mb-2">
                  {groupe}
                </button>
                <div className="space-y-1.5">
                  {permissions.map(p => (
                    <label key={p.code}
                      className="flex items-start gap-2 text-sm cursor-pointer">
                      <input type="checkbox" className="mt-0.5"
                        checked={cochees.has(p.code)}
                        onChange={() => basculer(p.code)} />
                      <span>
                        {p.libelle}
                        <span className="text-muted-foreground text-xs ml-2">
                          {p.code}
                        </span>
                      </span>
                    </label>
                  ))}
                </div>
              </div>
            ))}
          </div>

          <div className="flex gap-2 pt-2">
            <Button variant="outline" onClick={onFermer}
              disabled={chargement} className="flex-1">Annuler</Button>
            <Button onClick={enregistrer}
              disabled={chargement || (!role && !nom.trim())}
              className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Enregistrer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  L'onglet
// =====================================================================

export function OngletRoles() {
  const [roles, setRoles] = useState<Role[]>([]);
  const [catalogue, setCatalogue] = useState<Permission[]>([]);
  const [chargement, setChargement] = useState(true);
  const [erreur, setErreur] = useState<string | null>(null);
  const [modal, setModal] = useState<{ ouvert: boolean; role: Role | null }>(
    { ouvert: false, role: null },
  );

  async function charger() {
    setChargement(true);
    setErreur(null);
    try {
      const [r, c] = await Promise.all([
        invoke<Role[]>("lire_roles"),
        invoke<Permission[]>("lire_catalogue_permissions"),
      ]);
      setRoles(r);
      setCatalogue(c);
    } catch (e) {
      setErreur(String(e));
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  async function supprimer(role: Role) {
    const ok = await confirm(
      `Supprimer le rôle « ${role.nom} » ?`,
      { title: "Supprimer un rôle", kind: "warning" },
    );
    if (!ok) return;
    try {
      await invoke("supprimer_role", { roleId: role.id });
      charger();
    } catch (e) {
      await message(String(e), { title: "Impossible", kind: "error" });
    }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  if (erreur) return (
    <p className="text-sm text-red-600 py-4">{erreur}</p>
  );

  return (
    <div className="space-y-4 max-w-3xl">
      <div className="flex items-start justify-between gap-4">
        <p className="text-sm text-muted-foreground">
          Un rôle décide de ce qu'une personne peut faire. Les rôles
          livrés sont des points de départ : ajustez-les, ou créez les
          vôtres.
        </p>
        <Button size="sm" onClick={() => setModal({ ouvert: true, role: null })}>
          <Plus className="h-4 w-4 mr-1" /> Nouveau
        </Button>
      </div>

      <div className="space-y-2">
        {roles.map(r => (
          <div key={r.id}
            className="border border-border rounded-lg px-4 py-3">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                  {r.protege
                    ? <Lock className="h-3.5 w-3.5 text-amber-600" />
                    : r.acces_total
                      ? <ShieldCheck className="h-3.5 w-3.5 text-primary" />
                      : <Shield className="h-3.5 w-3.5 text-muted-foreground" />}
                  <span className="text-sm font-medium">{r.nom}</span>
                  {r.acces_total && <Badge variant="default">Accès complet</Badge>}
                  {r.protege && <Badge variant="outline">Protégé</Badge>}
                </div>
                {r.description && (
                  <p className="text-xs text-muted-foreground mt-1">
                    {r.description}
                  </p>
                )}
                <p className="text-xs text-muted-foreground mt-1
                              flex items-center gap-3">
                  <span>
                    {r.permissions.length} permission
                    {r.permissions.length > 1 ? "s" : ""}
                  </span>
                  <span className="flex items-center gap-1">
                    <Users className="h-3 w-3" />
                    {r.nb_utilisateurs}
                  </span>
                </p>
              </div>

              <div className="flex items-center gap-1 shrink-0">
                {/* Le rôle protégé n'offre AUCUN bouton : un bouton qui
                    échoue à tous les coups est pire que pas de bouton. */}
                {!r.protege && (
                  <>
                    <Button variant="outline" size="sm"
                      onClick={() => setModal({ ouvert: true, role: r })}>
                      Modifier
                    </Button>
                    {r.nb_utilisateurs === 0 && !r.acces_total && (
                      <Button variant="ghost" size="sm"
                        onClick={() => supprimer(r)}
                        title="Supprimer">
                        <Trash2 className="h-3.5 w-3.5 text-red-500" />
                      </Button>
                    )}
                  </>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>

      <p className="text-xs text-muted-foreground">
        Un rôle « accès complet » reçoit aussi les fonctions ajoutées par
        les mises à jour. Les autres ne reçoivent que ce qui est coché —
        c'est voulu : une nouveauté ne doit s'ouvrir à personne sans que
        vous l'ayez décidé.
      </p>

      <ModalRole
        ouvert={modal.ouvert}
        role={modal.role}
        catalogue={catalogue}
        onFermer={() => setModal({ ouvert: false, role: null })}
        onEnregistre={() => { setModal({ ouvert: false, role: null }); charger(); }}
      />
    </div>
  );
}
