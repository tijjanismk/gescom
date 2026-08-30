import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ShoppingCart, Package, Users, Wallet,
  BarChart3, Settings, Menu, X, Store,
  ShoppingBag, Truck, RotateCcw, LogOut,
  Lock, ChevronDown, FileText,
  MessageCircle, BarChart2, BookOpen, ArrowLeftRight, Warehouse,
  FileCheck,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { UtilisateurConnecte } from "@/pages/PageLogin";

const NAV_PATRON = [
  { nom: "Tableau de bord", icone: BarChart3,      href: "dashboard"   },
  { nom: "Ventes",          icone: ShoppingCart,    href: "ventes"      },
  { nom: "Achats",          icone: ShoppingBag,     href: "achats"      },
  { nom: "Pièces",          icone: FileText,        href: "pieces"      },
  { nom: "Stock",           icone: Package,         href: "stock"       },
  { nom: "Clients",         icone: Users,           href: "clients"     },
  { nom: "Fournisseurs",    icone: Truck,           href: "fournisseurs"},
  { nom: "Caisse",          icone: Wallet,          href: "caisse"      },
  { nom: "Retours",         icone: RotateCcw,       href: "retours"     },
  { nom: "Transferts",      icone: ArrowLeftRight,  href: "transferts"  },
  { nom: "Chèques",         icone: FileCheck,       href: "cheques"     },
  { nom: "Relances",        icone: MessageCircle,   href: "relances"    },
  { nom: "Journal",         icone: BookOpen,        href: "journal"     },
  { nom: "Rapports",        icone: BarChart2,       href: "rapports"    },
  { nom: "Paramètres",      icone: Settings,        href: "parametres"  },
];

const NAV_EMPLOYE = [
  { nom: "Ventes",          icone: ShoppingCart,    href: "ventes"      },
  { nom: "Achats",          icone: ShoppingBag,     href: "achats"      },
  { nom: "Pièces",          icone: FileText,        href: "pieces"      },
  { nom: "Stock",           icone: Package,         href: "stock"       },
  { nom: "Clients",         icone: Users,           href: "clients"     },
  { nom: "Retours",         icone: RotateCcw,       href: "retours"     },
];

interface Depot { id: string; nom: string; est_defaut?: boolean; }

interface LayoutProps {
  children: React.ReactNode;
  pageActive: string;
  onNaviguer: (page: string) => void;
  /** null = vue consolidée, tous les dépôts. */
  depotActif: string | null;
  onChangerDepot: (id: string | null) => void;
  role: string;
  utilisateur: UtilisateurConnecte;
  onChangerMdp: () => void;
  onDeconnecter: () => void;
}

export function Layout({
  children, pageActive, onNaviguer, depotActif, onChangerDepot, role,
  utilisateur, onChangerMdp, onDeconnecter,
}: LayoutProps) {
  const [sidebarOuverte, setSidebarOuverte] = useState(true);
  const [menuUtilisateur, setMenuUtilisateur] = useState(false);
  const [depots, setDepots] = useState<Depot[]>([]);

  useEffect(() => {
    invoke<Depot[]>("lire_depots").then(setDepots).catch(console.error);
  }, []);

  // Le sélecteur n'a de sens qu'avec plusieurs dépôts. Avec un seul,
  // l'afficher ajouterait une décision inutile au quotidien.
  const multiDepot = depots.length > 1;

  const navigation = role === "patron" ? NAV_PATRON : NAV_EMPLOYE;

  return (
    <div className="flex h-screen bg-background overflow-hidden">

      {/* Sidebar */}
      <aside className={cn(
        "flex flex-col bg-card border-r border-border transition-all duration-300",
        sidebarOuverte ? "w-56" : "w-14"
      )}>

        {/* En-tête */}
        <div className="flex items-center justify-between h-14 px-3 border-b border-border">
          {sidebarOuverte && (
            <div className="flex items-center gap-2">
              <Store className="h-5 w-5 text-primary" />
              <span className="font-semibold text-sm">Gescom</span>
            </div>
          )}
          <button
            onClick={() => setSidebarOuverte(!sidebarOuverte)}
            className="p-1.5 rounded-md hover:bg-accent transition-colors">
            {sidebarOuverte ? <X className="h-4 w-4" /> : <Menu className="h-4 w-4" />}
          </button>
        </div>

        {/* Dépôt actif — masqué s'il n'y a qu'un seul dépôt */}
        {multiDepot && sidebarOuverte && (
          <div className="px-2 pt-2">
            <label className="text-[10px] uppercase text-muted-foreground
                              px-1 mb-1 flex items-center gap-1">
              <Warehouse className="h-3 w-3" /> Dépôt
            </label>
            <select
              value={depotActif ?? ""}
              onChange={e => onChangerDepot(e.target.value || null)}
              className="w-full h-8 text-xs border border-border rounded-md
                         bg-background px-2 focus:outline-none
                         focus:ring-1 focus:ring-primary">
              <option value="">Tous les dépôts</option>
              {depots.map(d => (
                <option key={d.id} value={d.id}>{d.nom}</option>
              ))}
            </select>
          </div>
        )}

        {/* Navigation */}
        <nav className="flex-1 p-2 space-y-0.5 overflow-auto">
          {navigation.map(item => {
            const Icone = item.icone;
            const estActif = pageActive === item.href;
            return (
              <button key={item.href} onClick={() => onNaviguer(item.href)}
                className={cn(
                  "w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-sm transition-colors",
                  estActif
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                )}>
                <Icone className="h-4 w-4 shrink-0" />
                {sidebarOuverte && <span className="truncate">{item.nom}</span>}
              </button>
            );
          })}
        </nav>

        {/* Pied — utilisateur */}
        <div className="border-t border-border">
          <div className="relative">
            <button
              onClick={() => setMenuUtilisateur(!menuUtilisateur)}
              className={cn(
                "w-full flex items-center gap-2 p-3 hover:bg-accent transition-colors",
                sidebarOuverte ? "justify-between" : "justify-center"
              )}>
              <div className="flex items-center gap-2 min-w-0">
                <div className="w-6 h-6 rounded-full bg-primary flex items-center justify-center shrink-0">
                  <span className="text-xs text-primary-foreground font-medium">
                    {utilisateur.nom[0].toUpperCase()}
                  </span>
                </div>
                {sidebarOuverte && (
                  <div className="min-w-0 text-left">
                    <p className="text-xs font-medium truncate">{utilisateur.nom}</p>
                    <p className="text-xs text-muted-foreground capitalize">{role}</p>
                  </div>
                )}
              </div>
              {sidebarOuverte && (
                <ChevronDown className={cn(
                  "h-3.5 w-3.5 text-muted-foreground transition-transform shrink-0",
                  menuUtilisateur && "rotate-180"
                )} />
              )}
            </button>

            {menuUtilisateur && (
              <div className={cn(
                "absolute bottom-full left-0 right-0 bg-card border border-border rounded-t-md shadow-md",
                !sidebarOuverte && "left-14 right-auto w-40"
              )}>
                <button
                  onClick={() => { setMenuUtilisateur(false); onChangerMdp(); }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm
                             hover:bg-accent transition-colors">
                  <Lock className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>Changer mot de passe</span>
                </button>
                <button
                  onClick={() => { setMenuUtilisateur(false); onDeconnecter(); }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm
                             text-destructive hover:bg-accent transition-colors">
                  <LogOut className="h-3.5 w-3.5" />
                  <span>Se déconnecter</span>
                </button>
              </div>
            )}
          </div>
        </div>
      </aside>

      {/* Contenu */}
      <main className="flex-1 flex flex-col overflow-hidden"
        onClick={() => setMenuUtilisateur(false)}>
        {children}
      </main>
    </div>
  );
}
