import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Store, Eye, EyeOff, Loader2, Lock, User } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export interface UtilisateurConnecte {
  id: string;
  nom: string;
  role: "patron" | "employe" | "lecture";
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
      const utilisateur = await invoke<UtilisateurConnecte>("connexion", {
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

        {/* Comptes par défaut — visibles uniquement en développement */}
        {import.meta.env.DEV && (
        <div className="mt-6 p-3 bg-muted rounded-lg text-xs text-muted-foreground">
          <p className="font-medium mb-1">Comptes par défaut :</p>
          <p>Patron : <span className="font-mono">admin</span> / <span className="font-mono">admin123</span></p>
          <p>Employé : <span className="font-mono">employe</span> / <span className="font-mono">employe123</span></p>
        </div>
        )}
      </div>
    </div>
  );
}