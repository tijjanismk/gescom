import { useState } from "react";
import { appeler as invoke, connecterServeur, enReseau, etatReseau } from "@/lib/pont";
import { Store, Eye, EyeOff, Loader2, Lock, User } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export interface UtilisateurConnecte {
  id: string;
  nom: string;
  /**
   * Le nom du rôle, pour l'afficher. Il n'est PLUS une énumération :
   * le commerçant crée les rôles qu'il veut, et l'écran ne doit rien
   * décider à partir de ce texte — c'est `permissions` qui tranche.
   */
  role: string;
  /** Ce que cette personne a le droit de faire. Voir `lib/droits.ts`. */
  permissions: string[];
  doit_changer_mdp: boolean;
}

interface PageLoginProps {
  onConnecte: (utilisateur: UtilisateurConnecte) => void;
}

export function PageLogin({ onConnecte }: PageLoginProps) {
  const [identifiant, setIdentifiant] = useState("");
  const [motDePasse, setMotDePasse] = useState("");
  const [visible, setVisible] = useState(false);
  const [chargement, setChargement] = useState(false);
  const [erreur, setErreur] = useState("");

  async function handleConnexion(e?: React.FormEvent) {
    e?.preventDefault();
    if (!identifiant.trim() || !motDePasse) return;

    setChargement(true);
    setErreur("");

    try {
      // En poste caisse, c'est le SERVEUR qui authentifie : la base des
      // utilisateurs est chez lui. Vérifier ici contre une base locale
      // vide refuserait tout le monde, et vérifier contre une base
      // locale pleine laisserait entrer avec un mot de passe que le
      // patron a peut-être changé depuis.
      const utilisateur = enReseau()
        ? await (async () => {
            const id = await connecterServeur(identifiant.trim(), motDePasse);
            return {
              id: id.utilisateur_id,
              nom: id.utilisateur_nom,
              role: id.role,
              permissions: id.permissions ?? [],
              doit_changer_mdp: id.doit_changer_mdp,
            };
          })()
        : await invoke<UtilisateurConnecte>("connexion", {
            identifiant: identifiant.trim(),
            motDePasse: motDePasse,
          });
      onConnecte(utilisateur);
    } catch (err) {
      setErreur(typeof err === "string" ? err : "Identifiant ou mot de passe incorrect");
      setMotDePasse("");
    } finally {
      setChargement(false);
    }
  }

  return (
    <div className="h-screen flex flex-col items-center justify-center bg-background p-8">

      {enReseau() && (
        <div className="mb-4 rounded-md border border-border bg-muted/40 px-3 py-1.5
                        text-xs text-muted-foreground">
          Poste caisse — connexion au serveur{" "}
          <span className="font-mono">{etatReseau().serveur}</span>
        </div>
      )}

      {/* Logo */}
      <div className="flex items-center gap-3 mb-10">
        <div className="w-12 h-12 rounded-xl bg-primary flex items-center justify-center">
          <Store className="h-6 w-6 text-primary-foreground" />
        </div>
        <div>
          <h1 className="text-2xl font-bold">Gescom</h1>
          <p className="text-xs text-muted-foreground">Gestion commerciale</p>
        </div>
      </div>

      {/* Formulaire */}
      <div className="w-full max-w-sm">
        <div className="space-y-4">

          <div>
            <Label htmlFor="identifiant">Pseudo ou email</Label>
            <div className="relative mt-1">
              <User className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                id="identifiant"
                value={identifiant}
                onChange={e => setIdentifiant(e.target.value)}
                placeholder="admin"
                className="pl-9"
                autoFocus
                autoComplete="username"
                onKeyDown={e => e.key === "Enter" && handleConnexion()}
              />
            </div>
          </div>

          <div>
            <Label htmlFor="mot_de_passe">Mot de passe</Label>
            <div className="relative mt-1">
              <Lock className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                id="mot_de_passe"
                type={visible ? "text" : "password"}
                value={motDePasse}
                onChange={e => setMotDePasse(e.target.value)}
                placeholder="••••••••"
                className="pl-9 pr-9"
                autoComplete="current-password"
                onKeyDown={e => e.key === "Enter" && handleConnexion()}
              />
              <button
                type="button"
                onClick={() => setVisible(!visible)}
                className="absolute right-3 top-2.5 text-muted-foreground hover:text-foreground"
              >
                {visible
                  ? <EyeOff className="h-4 w-4" />
                  : <Eye className="h-4 w-4" />
                }
              </button>
            </div>
          </div>

          {erreur && (
            <p className="text-sm text-destructive text-center">{erreur}</p>
          )}

          <Button
            onClick={() => handleConnexion()}
            disabled={!identifiant.trim() || !motDePasse || chargement}
            className="w-full"
            size="lg"
          >
            {chargement
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : "Se connecter"
            }
          </Button>
        </div>
      </div>
    </div>
  );
}