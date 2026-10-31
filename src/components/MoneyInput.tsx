import { useRef } from "react";
import { cn } from "@/lib/utils";
import { chiffresDuMontant, lireMontant } from "@/lib/montant";

interface MoneyInputProps {
  value: string;
  onChange: (val: string) => void;
  placeholder?: string;
  className?: string;
  autoFocus?: boolean;
  disabled?: boolean;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  id?: string;
  "aria-label"?: string;
}

export function MoneyInput({
  value, onChange, placeholder = "0",
  className, autoFocus, disabled, onKeyDown, id, "aria-label": ariaLabel,
}: MoneyInputProps) {
  const ref = useRef<HTMLInputElement>(null);

  // Afficher avec séparateur de milliers
  function formaterAffichage(raw: string): string {
    const num = raw.replace(/\D/g, "");
    if (!num) return "";
    return new Intl.NumberFormat("fr-ML").format(parseInt(num));
  }

  // Le franc CFA n'a pas de centimes : « 15.000 » collé ou tapé vaut
  // 15 000 (lib/montant.ts). Un texte qui n'est pas un montant ne
  // change rien.
  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const tape = e.target.value;
    const brut = chiffresDuMontant(tape);
    if (brut !== "" || tape.trim() === "") {
      onChange(brut);
    }
  }

  const affichage = formaterAffichage(value);

  return (
    <input
      ref={ref}
      id={id}
      aria-label={ariaLabel}
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

// Utilitaire : un montant tapé en entier (0 si ce n'en est pas un).
// « 15.000 » vaut 15 000 : le franc CFA n'a pas de centimes.
export function parseMontant(val: string): number {
  return lireMontant(val) ?? 0;
}
