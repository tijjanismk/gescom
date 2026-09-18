// lib/onglets-parametres.ts — les onglets de Paramètres, et le droit
// que chacun demande.
//
// Hors du composant pour que la palette de commandes (Layout) puisse
// proposer « Paramètres › Rôles » avec le même droit que l'onglet, sans
// importer l'écran entier — et sans casser le rechargement à chaud, qui
// veut qu'un fichier de composant n'exporte que des composants.
//
// L'onglet s'affiche si la personne a la permission. Rien de plus :
// c'est la même liste blanche que le noyau, vue de l'écran.

import {
  Package, Tag, Building2, Users, HardDrive, ShoppingCart,
  Percent, Banknote, XCircle, Clock, Warehouse, Barcode,
  LayoutTemplate, FileSpreadsheet, Network, Shield,
} from "lucide-react";

export const ONGLETS_PARAMETRES = [
  { key: "societe",       label: "Société",       icone: Building2,       droit: "parametres:modifier" },
  { key: "depots",        label: "Magasins",        icone: Warehouse,       droit: "depots:gerer" },
  { key: "articles",      label: "Articles",      icone: Package,         droit: "articles:creer" },
  { key: "codesbarres",   label: "Codes-barres",  icone: Barcode,         droit: "articles:creer" },
  { key: "importexport",  label: "Import/Export", icone: FileSpreadsheet, droit: "parametres:modifier" },
  { key: "categories",    label: "Catégories",    icone: Tag,             droit: "articles:creer" },
  { key: "ventes",        label: "Ventes",        icone: ShoppingCart,    droit: "parametres:modifier" },
  { key: "utilisateurs",  label: "Utilisateurs",  icone: Users,           droit: "utilisateurs:gerer" },
  { key: "roles",         label: "Rôles",         icone: Shield,          droit: "utilisateurs:gerer" },
  { key: "sauvegarde",    label: "Sauvegarde",    icone: HardDrive,       droit: "sauvegarde:lancer" },
  { key: "reseau",        label: "Réseau",        icone: Network,         droit: "postes:gerer" },
  { key: "tva",           label: "TVA",           icone: Percent,         droit: "chantiers:gerer" },
  { key: "dettes",        label: "Dettes fourn.", icone: Banknote,        droit: "fournisseurs:regler" },
  { key: "irrecouvrable", label: "Irrécouvrable", icone: XCircle,         droit: "chantiers:gerer" },
  { key: "avoirs",        label: "Avoirs",        icone: Clock,           droit: "avoirs:gerer" },
  // Les modèles ouvrent l'atelier EN PLEIN ÉCRAN : il lui faut les
  // trois colonnes et l'aperçu à taille réelle. L'onglet n'est donc
  // qu'une porte — les autres onglets s'effacent derrière.
  { key: "modeles",       label: "Modèles de documents", icone: LayoutTemplate, droit: "modeles:gerer" },
];
