#!/bin/bash
# Banc d'essai de l'ecran : le vrai serveur sur une base SQLite jetable
# (donnees de demo) + l'ecran servi par Vite, pour les parcours
# Playwright de ce dossier (a1.mjs, a2.mjs…).
#
#   outils/banc/lancer.sh          relance sur la base existante
#   outils/banc/lancer.sh neuf     repart d'une base vide
#
# Linux ou Git Bash. La base et les captures vivent dans .travail/,
# ignore par git.
set -e
RACINE="$(cd "$(dirname "$0")/../.." && pwd)"
TRAVAIL="$RACINE/outils/banc/.travail"
mkdir -p "$TRAVAIL/captures"
BASE="$TRAVAIL/essai.db"
PORT_SERVEUR="${PORT_SERVEUR:-7300}"
PORT_ECRAN="${PORT_ECRAN:-1420}"
# Arreter l'instance precedente (par son port, pas par son nom).
for p in $PORT_SERVEUR $PORT_ECRAN; do
  pid=$(lsof -ti tcp:$p 2>/dev/null || true)
  [ -n "$pid" ] && kill $pid 2>/dev/null || true
done
sleep 1
if [ "$1" = "neuf" ]; then rm -f "$BASE" "$BASE-wal" "$BASE-shm"; fi
(cd "$RACINE/src-tauri" && cargo build -q -p gescom-serveur)
cd "$RACINE"
GESCOM_DEMO=1 nohup src-tauri/target/debug/gescom-serveur --hote 127.0.0.1 --port $PORT_SERVEUR \
  --base "$BASE" --sauvegardes "$TRAVAIL/sauvegardes" > "$TRAVAIL/serveur.out" 2>&1 &
nohup npx vite --port $PORT_ECRAN --strictPort --host 127.0.0.1 > "$TRAVAIL/vite.out" 2>&1 &
for i in $(seq 1 60); do
  if curl -s 127.0.0.1:$PORT_SERVEUR/sante >/dev/null && curl -s 127.0.0.1:$PORT_ECRAN >/dev/null; then
    echo PRET; exit 0
  fi
  sleep 1
done
echo "PAS PRET"; tail -5 "$TRAVAIL/serveur.out" "$TRAVAIL/vite.out"; exit 1
