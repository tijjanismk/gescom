import { useState } from "react";
import {
  ShoppingCart, Package, Users, Wallet,
  BarChart3, Settings, Menu, X, Store,
  ShoppingBag, Truck,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { Role } from "@/App";

// Navigation selon le rôle
const NAV_PATRON = [
  { nom: "Tableau de bord", icone: BarChart3,    href: "dashboard" },
  { nom: "Ventes",          icone: ShoppingCart,  href: "ventes" },
  { nom: "Achats",          icone: ShoppingBag,   href: "achats" },
  { nom: "Stock",           icone: Package,       href: "stock" },
  { nom: "Clients",         icone: Users,         href: "clients" },
  { nom: "Fournisseurs",    icone: Truck,         href: "fournisseurs" },
  { nom: "Caisse",          icone: Wallet,        href: "caisse" },
  { nom: "Paramètres",      icone: Settings,      href: "parametres" },
];

const NAV_EMPLOYE = [
  { nom: "Ventes",          icone: ShoppingCart,  href: "ventes" },
  { nom: "Achats",          icone: ShoppingBag,   href: "achats" },
  { nom: "Stock",           icone: Package,       href: "stock" },
  { nom: "Clients",         icone: Users,         href: "clients" },
];

interface LayoutProps {
  children: React.ReactNode;
  pageActive: string;
  onNaviguer: (page: string) => void;
  role: Role;
}

export function Layout({ children, pageActive, onNaviguer, role }: LayoutProps) {
  const [sidebarOuverte, setSidebarOuverte] = useState(true);
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
            className="p-1.5 rounded-md hover:bg-accent transition-colors"
          >
            {sidebarOuverte
              ? <X className="h-4 w-4" />
              : <Menu className="h-4 w-4" />
            }
          </button>
        </div>

        {/* Navigation */}
        <nav className="flex-1 p-2 space-y-0.5">
          {navigation.map(item => {
            const Icone = item.icone;
            const estActif = pageActive === item.href;
            return (
              <button
                key={item.href}
                onClick={() => onNaviguer(item.href)}
                className={cn(
                  "w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-sm transition-colors",
                  estActif
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                )}
              >
                <Icone className="h-4 w-4 shrink-0" />
                {sidebarOuverte && <span className="truncate">{item.nom}</span>}
              </button>
            );
          })}
        </nav>

        {/* Pied — profil actif */}
        <div className="p-3 border-t border-border">
          {sidebarOuverte ? (
            <div className="text-xs text-muted-foreground">
              <p className="font-medium text-foreground capitalize">{role}</p>
              <p>Dépôt principal</p>
            </div>
          ) : (
            <div className="w-6 h-6 rounded-full bg-primary flex items-center justify-center">
              <span className="text-xs text-primary-foreground font-medium capitalize">
                {role[0].toUpperCase()}
              </span>
            </div>
          )}
        </div>
      </aside>

      {/* Contenu principal */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {children}
      </main>
    </div>
  );
}