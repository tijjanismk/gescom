import { useState } from "react";
import {
  ShoppingCart,
  Package,
  Users,
  Wallet,
  BarChart3,
  Settings,
  Menu,
  X,
  Store,
} from "lucide-react";
import { cn } from "@/lib/utils";

// Les modules de navigation — on en active d'autres plus tard
const navigation = [
  { nom: "Tableau de bord", icone: BarChart3, href: "dashboard", actif: true },
  { nom: "Ventes",          icone: ShoppingCart, href: "ventes", actif: true },
  { nom: "Stock",           icone: Package, href: "stock", actif: true },
  { nom: "Clients",         icone: Users, href: "clients", actif: true },
  { nom: "Caisse",          icone: Wallet, href: "caisse", actif: true },
  { nom: "Paramètres",      icone: Settings, href: "parametres", actif: true },
];

interface LayoutProps {
  children: React.ReactNode;
  pageActive: string;
  onNaviguer: (page: string) => void;
}

export function Layout({ children, pageActive, onNaviguer }: LayoutProps) {
  const [sidebarOuverte, setSidebarOuverte] = useState(true);

  return (
    <div className="flex h-screen bg-background overflow-hidden">

      {/* Sidebar */}
      <aside
        className={cn(
          "flex flex-col bg-card border-r border-border transition-all duration-300",
          sidebarOuverte ? "w-56" : "w-14"
        )}
      >
        {/* En-tête sidebar */}
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
          {navigation.map((item) => {
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
                {sidebarOuverte && (
                  <span className="truncate">{item.nom}</span>
                )}
              </button>
            );
          })}
        </nav>

        {/* Pied sidebar — info utilisateur */}
        <div className="p-3 border-t border-border">
          {sidebarOuverte ? (
            <div className="text-xs text-muted-foreground">
              <p className="font-medium text-foreground">Patron</p>
              <p>Dépôt principal</p>
            </div>
          ) : (
            <div className="w-6 h-6 rounded-full bg-primary flex items-center justify-center">
              <span className="text-xs text-primary-foreground font-medium">P</span>
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