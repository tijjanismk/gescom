import { useRef } from "react";
import { cn } from "@/lib/utils";

interface MoneyInputProps {
  value: string;
  onChange: (val: string) => void;
  placeholder?: string;
  className?: string;
  autoFocus?: boolean;
  disabled?: boolean;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
}

export function MoneyInput({
  value, onChange, placeholder = "0",
  className, autoFocus, disabled, onKeyDown,
}: MoneyInputProps) {
  const ref = useRef<HTMLInputElement>(null);

  // Afficher avec séparateur de milliers
  function formaterAffichage(raw: string): string {
    const num = raw.replace(/\D/g, "");
    if (!num) return "";
    return new Intl.NumberFormat("fr-ML").format(parseInt(num));
  }

  // Extraire la valeur brute depuis la valeur affichée
  function extraireValeur(affiche: string): string {
    return affiche.replace(/\s/g, "").replace(/\u202f/g, "").replace(/,/g, "");
  }

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const brut = extraireValeur(e.target.value);
    if (brut === "" || /^\d+$/.test(brut)) {
      onChange(brut);
    }
  }

  const affichage = formaterAffichage(value);

  return (
    <input
      ref={ref}
      type="text"
      inputMode="numeric"
      value={affichage}
      onChange={handleChange}
      placeholder={placeholder}
      autoFocus={autoFocus}
      disabled={disabled}
      onKeyDown={onKeyDown}
      className={cn(
        "flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1",
        "text-sm shadow-sm transition-colors text-right font-medium tracking-wide",
        "placeholder:text-muted-foreground",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
        "disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
    />
  );
}

// Utilitaire : parse un string MoneyInput en entier
export function parseMontant(val: string): number {
  const brut = val.replace(/\s/g, "").replace(/\u202f/g, "").replace(/,/g, "");
  return parseInt(brut) || 0;
}
