//! Hook React pour la détection du scanner de code-barres.
//!
//! Les scanners émulent un clavier — ils tapent les caractères très vite
//! (< 50ms entre chaque touche) puis envoient Enter.
//! Ce hook distingue une saisie scanner d'une saisie humaine normale.

import { useEffect, useRef, useCallback } from "react";

interface UseScannerOptions {
  actif: boolean;
  onScan: (code: string) => void;
  /** Délai max entre deux touches pour considérer que c'est un scan (ms) */
  delaiMax?: number;
  /** Longueur minimale d'un code pour être traité */
  longueurMin?: number;
}

export function useScanner({
  actif,
  onScan,
  delaiMax = 80,
  longueurMin = 3,
}: UseScannerOptions) {
  const bufferRef = useRef<string>("");
  const derniereToucheRef = useRef<number>(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!actif) return;

    // Ignorer si une modal est ouverte ou si le focus est sur un input normal
    const target = e.target as HTMLElement;
    const tagName = target.tagName.toLowerCase();

    // Si l'utilisateur tape dans un champ de recherche → ignorer
    if (tagName === "input" || tagName === "textarea") return;

    const maintenant = Date.now();
    const delai = maintenant - derniereToucheRef.current;
    derniereToucheRef.current = maintenant;

    // Réinitialiser le buffer si trop de temps entre deux touches
    if (delai > delaiMax * 3 && bufferRef.current.length > 0) {
      bufferRef.current = "";
    }

    if (e.key === "Enter") {
      const code = bufferRef.current.trim();
      bufferRef.current = "";

      if (code.length >= longueurMin && delai < delaiMax * 2) {
        e.preventDefault();
        onScan(code);
      }
      return;
    }

    // Accumuler les caractères si arrivée rapide (scan) ou buffer déjà en cours
    if (e.key.length === 1) {
      if (delai < delaiMax || bufferRef.current.length > 0) {
        bufferRef.current += e.key;

        // Timer de sécurité — vider le buffer après 500ms sans Enter
        if (timerRef.current) clearTimeout(timerRef.current);
        timerRef.current = setTimeout(() => {
          bufferRef.current = "";
        }, 500);
      }
    }
  }, [actif, onScan, delaiMax, longueurMin]);

  useEffect(() => {
    if (!actif) return;
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [actif, handleKeyDown]);
}
