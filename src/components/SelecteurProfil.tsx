import { useState } from "react";
import { Store, Shield, User } from "lucide-react";

interface Profil {
  role: "patron" | "employe";
  nom: string;
  description: string;
  icone: React.ComponentType<{ className?: string }>;
  couleur: string;
}

const PROFILS: Profil[] = [
  {
    role: "patron",
    nom: "Patron",
    description: "Accès complet — ventes, achats, caisse, prix d'achat, paramètres",
    icone: Shield,
    couleur: "border-primary bg-primary/5 hover:bg-primary/10",
  },
  {
    role: "employe",
    nom: "Employé",
    description: "Ventes et réception marchandise — sans accès aux prix d'achat ni à la caisse complète",
    icone: User,
    couleur: "border-border bg-muted/30 hover:bg-muted/60",
  },
];

interface SelecteurProfilProps {
  onSelectionner: (role: "patron" | "employe") => void;
}

export function SelecteurProfil({ onSelectionner }: SelecteurProfilProps) {
  const [selectionne, setSelectionne] = useState<"patron" | "employe" | null>(null);

  function handleContinuer() {
    if (selectionne) onSelectionner(selectionne);
  }

  return (
    <div className="h-screen flex flex-col items-center justify-center bg-background p-8">

      {/* Logo */}
      <div className="flex items-center gap-3 mb-10">
        <div className="w-10 h-10 rounded-xl bg-primary flex items-center justify-center">
          <Store className="h-5 w-5 text-primary-foreground" />
        </div>
        <div>
          <h1 className="text-xl font-bold">Gescom</h1>
          <p className="text-xs text-muted-foreground">Gestion commerciale</p>
        </div>
      </div>

      {/* Question */}
      <p className="text-sm text-muted-foreground mb-6">
        Qui utilise l'application ?
      </p>

      {/* Cartes de profil */}
      <div className="flex gap-4 mb-8">
        {PROFILS.map(profil => {
          const Icone = profil.icone;
          const estSelectionne = selectionne === profil.role;
          return (
            <button
              key={profil.role}
              onClick={() => setSelectionne(profil.role)}
              className={`
                w-52 p-5 rounded-xl border-2 text-left transition-all
                ${profil.couleur}
                ${estSelectionne ? "ring-2 ring-primary ring-offset-2" : ""}
              `}
            >
              <div className={`
                w-10 h-10 rounded-lg flex items-center justify-center mb-3
                ${estSelectionne ? "bg-primary text-primary-foreground" : "bg-muted"}
              `}>
                <Icone className="h-5 w-5" />
              </div>
              <p className="font-semibold text-sm mb-1">{profil.nom}</p>
              <p className="text-xs text-muted-foreground leading-relaxed">
                {profil.description}
              </p>
            </button>
          );
        })}
      </div>

      {/* Bouton continuer */}
      <button
        onClick={handleContinuer}
        disabled={!selectionne}
        className={`
          px-8 py-2.5 rounded-lg text-sm font-medium transition-all
          ${selectionne
            ? "bg-primary text-primary-foreground hover:bg-primary/90"
            : "bg-muted text-muted-foreground cursor-not-allowed"
          }
        `}
      >
        Continuer →
      </button>

      <p className="text-xs text-muted-foreground mt-8 text-center max-w-xs">
        Vous pourrez changer de profil en fermant et rouvrant l'application.
      </p>
    </div>
  );
}
