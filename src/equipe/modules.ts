// equipe/modules.ts — les écrans de Gescom Équipe et le droit que
// chacun demande (PLAN-EQUIPE). Hors du composant, comme
// lib/onglets-parametres.ts : le menu et l'accueil lisent la même liste.

import { Home, Users, Wallet, MessageSquareText, type LucideIcon } from "lucide-react";

export interface ModuleEquipe {
  cle: string;
  libelle: string;
  description: string;
  icone: LucideIcon;
  /** Au moins une de ces permissions ; vide = toute personne connectée. */
  droits: string[];
  /** L'étape du plan qui l'apporte, tant qu'il n'est pas là. */
  aVenir?: string;
}

export const MODULES: ModuleEquipe[] = [
  { cle: "accueil", libelle: "Accueil", description: "", icone: Home, droits: [] },
  {
    cle: "personnel", libelle: "Personnel", icone: Users,
    description: "Les fiches : qui fait quoi, comment on le paie. Sans contrat, c'est permis.",
    droits: ["personnel:gerer", "paie:preparer", "paie:valider"],
  },
  {
    cle: "paie", libelle: "Paie", icone: Wallet,
    description: "Avances, fiches du mois, versements depuis la caisse, bulletins.",
    droits: ["paie:preparer", "paie:valider", "personnel:avancer"], aVenir: "G",
  },
  {
    cle: "clients", libelle: "Suivi clients", icone: MessageSquareText,
    description: "Échanges, rappels, prospects, et tout ce qu'un client a fait avec la boutique.",
    droits: ["crm:suivre"], aVenir: "H",
  },
];
