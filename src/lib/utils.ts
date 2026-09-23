import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** « 1er mars 2026 », comme le serveur l'écrit dans ses refus (`coeur::dates::en_lettres`). */
export function jourEnLettres(iso: string): string {
  const d = new Date(iso.slice(0, 10) + "T12:00:00");
  if (isNaN(d.getTime())) return iso;
  const texte = d.toLocaleDateString("fr-ML", { day: "numeric", month: "long", year: "numeric" });
  return d.getDate() === 1 ? texte.replace(/^1 /, "1er ") : texte;
}

/** Le jour d'après, en `AAAA-MM-JJ`. */
export function lendemain(iso: string, jours = 1): string {
  const d = new Date(iso.slice(0, 10) + "T12:00:00");
  d.setDate(d.getDate() + jours);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
