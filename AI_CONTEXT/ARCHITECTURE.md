# Gescom — architecture

Application de gestion commerciale **local-first** pour commerçants de
Bamako. Les données restent dans la boutique : aucun cloud.

**v1** — un poste, une base SQLite, pas de serveur.
**v2 (en cours)** — deux exécutables : la fenêtre et
`gescom-serveur.exe`, qui détient la base pour plusieurs caisses. Le
monoposte reste le défaut et ne demande aucun service.
Voir [modules/reseau-v2.md](modules/reseau-v2.md).

## Stack

| Couche | Technologie |
|---|---|
| Coque | Tauri 2 (Rust), WebView2 |
| Métier | Rust — workspace `src-tauri/` : `noyau/`, `serveur/`, l'app |
| Réseau | HTTP/1.1 maison, un fil par connexion, **zéro dépendance** |
| Base | SQLite via rusqlite bundled, WAL, `foreign_keys=ON`, `busy_timeout=5000` |
| Écrans | React 18 + Vite + TypeScript — `src/` (~26 700 l., 71 fichiers) |
| UI | shadcn/ui + Tailwind 4 |

## Le seul pont entre les deux moitiés

Front et back ne partagent **aucun** type. Ils ne communiquent que par
`invoke("nom_commande", {...})` — qui passe désormais par
[src/lib/pont.ts](../src/lib/pont.ts), seul endroit qui sache si le code
métier tourne ici ou sur le serveur. Les 174 commandes sont énumérées dans
`generate_handler!` — [lib.rs:39-248](../src-tauri/src/lib.rs#L39).

Conséquence pratique : renommer une commande Rust ne casse **rien** à la
compilation. La rupture n'apparaît qu'à l'exécution, sur l'écran qui
l'appelle. Avant de renommer, chercher la chaîne dans `src/`.

Les types de retour sont redéclarés à la main côté front dans
[types-api.ts](../src/lib/types-api.ts) — et seulement pour une partie
des commandes. Le reste est typé en ligne dans les pages.

## Points d'entrée

- Rust : [main.rs](../src-tauri/src/main.rs) → `lib.rs::run()` — ouvre la
  base, applique les migrations, seede si vide, enregistre les commandes.
- Front : [main.tsx](../src/main.tsx) → [App.tsx](../src/App.tsx) —
  session localStorage 8 h, routeur maison (un `switch` sur `page_active`,
  pas de react-router).

## Découpage

```
src-tauri/
  noyau/src/      SANS Tauri — partagé par la fenêtre et le serveur
    coeur/          règles pures, sans I/O — 100 % testé
    persistance/    ouverture, schema.sql, migrations, journal
    protocole.rs    le contrat client/serveur
    sessions.rs postes.rs caisses.rs registre.rs portes.rs
  serveur/src/    gescom-serveur.exe — http.rs, api.rs, canal.rs, socle.rs
  src/            l'application Tauri
    commandes/      27 fichiers, une façade Tauri par domaine
    reseau.rs       mode monoposte / poste caisse
    seed.rs         jeu de données initial
    tests_multi_depot.rs   scénarios sur base en mémoire

src/
  pages/          18 écrans plein cadre, appelés par App.tsx
  components/     modales et onglets, montés dans les pages
  components/ui/  shadcn — primitives sans métier
  lib/            génération de documents HTML, helpers
```

## Deux invariants structurels

1. **Le stock et l'argent ne sortent qu'à `valider_facture`**
   ([pieces.rs:831](../src-tauri/src/commandes/pieces.rs#L831)). Bons de
   livraison, aperçus et suivi de livraison sont du document.
2. **Aucune opération d'argent sans session de caisse ouverte**
   (`utils::exiger_session_caisse`, code d'erreur `CAISSE_FERMEE`) — le
   garde-fou est posé sur 16 sites d'appel, tous listés dans
   [modules/commandes-argent.md](modules/commandes-argent.md).

## Où lire quoi

- Les 51 décisions numérotées (D1…D51) et leur justification :
  [CONTEXT.md](../CONTEXT.md). **Cette carte ne les recopie pas.**
- Le manuel utilisateur : [MANUEL.md](../MANUEL.md).
- Le graphe brut (imports, graphe inverse, orphelins) :
  [carte.json](carte.json) — généré, ne pas éditer.
