# Gescom — architecture

> **Où en est-on ?** → [ETAPES.md](ETAPES.md) : ce qui est fait, ce
> qui reste, et dans quel ordre. À relire au début de chaque séance.

Application de gestion commerciale **local-first** pour commerçants de
Bamako. Les données restent dans la boutique : aucun cloud.

**v1** — un poste, une base SQLite, pas de serveur. Produit livré, il
reste tel quel : la v2 ne cherche pas à le remplacer.

**v2 (en cours)** — deux exécutables : `gescom-serveur.exe`, qui détient
la base, et la fenêtre, qui est **toujours un client du serveur**.

### « Une seule machine » ne veut pas dire « sans serveur »

La boutique à une caisse installe **les deux sur le même ordinateur** :
le serveur tourne en fond, la fenêtre s'y connecte sur `127.0.0.1`. Ce
poste est à la fois le serveur et une caisse. Ajouter une deuxième
caisse plus tard ne change rien à l'installation du premier poste — on
branche la nouvelle, c'est tout.

⚠️ **Correction d'une lecture fautive.** Ce document a d'abord décrit la
v2 comme gardant un mode « monoposte » où la fenêtre ouvrait une base
SQLite locale, sans serveur. Ce n'était pas la demande, et cela a produit
deux chemins qui se contredisent — dont une panne réelle : un poste
caisse dont le cache est vidé se croyait monoposte, ouvrait une base
locale vide, et faisait conclure que les données avaient disparu.

Il n'y a **qu'un seul chemin** : le client parle au serveur.

Voir [modules/reseau-v2.md](modules/reseau-v2.md).

## Stack

| Couche | Technologie |
|---|---|
| Coque | Tauri 2 (Rust), WebView2 |
| Métier | Rust — workspace `src-tauri/` : `noyau/`, `serveur/`, l'app |
| Réseau | HTTP/1.1 maison, un fil par connexion, **zéro dépendance** |
| Base | SQLite via rusqlite bundled, WAL, `foreign_keys=ON`, `busy_timeout=5000` |
| Base (à venir) | PostgreSQL en **option du serveur** — [modules/postgresql.md](modules/postgresql.md) |
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
    reseau.rs       adresse du serveur, poste.json
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
  [_genere/carte.json](_genere/carte.json) — généré, ne pas lire : sa matière est dans ALERTES.md.
