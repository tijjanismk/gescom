// lib/session.ts — Persistance session avec localStorage + expiration 8h

export interface SessionUtilisateur {
  id: string;
  nom: string;
  role: string;
  doit_changer_mdp: boolean;
  connecte_le: number; // timestamp ms
}

const CLE = "gescom_session";
const DUREE_MS = 8 * 60 * 60 * 1000; // 8 heures

export function sauvegarderSession(user: Omit<SessionUtilisateur, "connecte_le">) {
  const session: SessionUtilisateur = {
    ...user,
    connecte_le: Date.now(),
  };
  localStorage.setItem(CLE, JSON.stringify(session));
}

export function lireSession(): SessionUtilisateur | null {
  try {
    const raw = localStorage.getItem(CLE);
    if (!raw) return null;
    const session: SessionUtilisateur = JSON.parse(raw);
    // Expiration après 8h
    if (Date.now() - session.connecte_le > DUREE_MS) {
      supprimerSession();
      return null;
    }
    return session;
  } catch {
    return null;
  }
}

export function supprimerSession() {
  localStorage.removeItem(CLE);
}

export function rafraichirSession() {
  const session = lireSession();
  if (session) {
    session.connecte_le = Date.now();
    localStorage.setItem(CLE, JSON.stringify(session));
  }
}