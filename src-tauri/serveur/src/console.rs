//! La console du serveur, servie par le serveur lui-même.
//!
//! ## Pourquoi une page et pas une fenêtre
//!
//! Une console noire sur le poste principal est ingérable pour un
//! commerçant : il ne sait pas ce qu'elle dit, et la fermer arrête la
//! boutique. Une vraie fenêtre demanderait une boîte à outils
//! graphique, donc un moteur de rendu embarqué dans un service censé
//! tourner sans écran.
//!
//! Le serveur parle déjà HTTP. Il sert donc sa propre page : le patron
//! ouvre `http://localhost:40000` sur le poste principal — ou l'adresse
//! du serveur depuis n'importe quelle caisse — et voit l'état, les
//! postes connectés, la dernière sauvegarde. Zéro dépendance ajoutée,
//! et ça marche depuis le téléphone posé sur le comptoir.
//!
//! ## Ce que la page montre sans mot de passe
//!
//! Exactement ce que `/sante` expose déjà : version, intégrité de la
//! base, nombre de postes connectés. C'est ce qu'un poste caisse
//! interroge AVANT d'avoir un jeton, et cela ne divulgue aucune donnée
//! de commerce.
//!
//! Le reste — le nom des postes, qui est connecté, révoquer une
//! session, lancer une sauvegarde — exige de s'identifier, avec les
//! mêmes identifiants que Gescom.

/// L'empreinte que la console annonce en se connectant.
///
/// Fixe, pour deux raisons. Elle evite d'ajouter une ligne de poste a
/// chaque ouverture du navigateur ; et elle laisse le serveur
/// reconnaitre la console pour lui donner le genre « console » plutot
/// que « caisse » — un poste caisse peut etre desactive, la console
/// non, sinon le patron s'enferme dehors depuis l'ecran meme d'ou il
/// aurait pu revenir.
pub const EMPREINTE: &str = "console-serveur";

/// La page, entière et autonome.
///
/// Aucune ressource extérieure : le poste d'une boutique de Bamako n'a
/// pas forcément Internet, et une console qui ne s'affiche pas le jour
/// d'une panne ne sert à rien.
pub fn page(port: u16) -> String {
    let style = STYLE;
    let script = SCRIPT;
    format!(
        r#"<!DOCTYPE html>
<html lang="fr"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Gescom Serveur</title>
<style>{style}</style>
</head><body>
<header>
  <h1>Gescom Serveur</h1>
  <span id="pastille" class="pastille attente">connexion…</span>
</header>

<main>
  <section class="carte">
    <h2>État</h2>
    <dl id="etat"><dt>Lecture…</dt><dd></dd></dl>
  </section>

  <section class="carte">
    <h2>Pour brancher une caisse</h2>
    <p class="aide">
      Sur le poste caisse : <b>Paramètres → Réseau → « Rechercher le
      serveur »</b>, ou saisir cette adresse.
    </p>
    <p><code id="adresse">…:{port}</code></p>
    <p class="aide" id="note-parefeu"></p>
  </section>

  <section class="carte" id="carte-identite">
    <h2>Supervision</h2>
    <p class="aide">
      Le nom des postes et les sessions ouvertes demandent de
      s'identifier — mêmes identifiants que Gescom.
    </p>
    <form id="form-connexion">
      <input id="identifiant" placeholder="Identifiant" autocomplete="username">
      <input id="motdepasse" type="password" placeholder="Mot de passe"
             autocomplete="current-password">
      <button type="submit">Se connecter</button>
    </form>
    <p id="erreur" class="erreur"></p>
  </section>

  <section class="carte cachee" id="carte-postes">
    <h2>Postes</h2>
    <div id="postes"></div>
  </section>

  <section class="carte cachee" id="carte-sessions">
    <h2>Sessions ouvertes</h2>
    <div id="sessions"></div>
  </section>

  <section class="carte cachee" id="carte-reseau">
    <h2>Réseau</h2>
    <dl id="reseau-etat"><dt>Lecture…</dt><dd></dd></dl>
    <div id="reseau-avertissements"></div>
    <h3>Sur les caisses, taper</h3>
    <div id="reseau-a-saisir" class="a-saisir"></div>
    <p class="aide">
      Ou, sur la caisse&nbsp;: Paramètres → Réseau → « Rechercher le
      serveur ». Le nom du poste vaut mieux que l'adresse IP&nbsp;: il ne
      change pas quand la box redistribue les adresses.
    </p>
    <h3>Changer le port ou l'écoute</h3>
    <p id="reseau-pourquoi" class="aide"></p>
    <form id="form-reseau" class="form-reseau">
      <label class="champ">Port
        <input id="reseau-port" type="number" min="40000" max="40100" step="1" inputmode="numeric">
      </label>
      <fieldset>
        <legend>Écouter</legend>
        <label class="choix"><input type="radio" name="ecoute" value="0.0.0.0">
          sur toutes les cartes réseau — les caisses le joignent</label>
        <label class="choix"><input type="radio" name="ecoute" value="127.0.0.1">
          sur ce poste seulement — aucune caisse ne le joint</label>
      </fieldset>
      <div><button type="submit" id="btn-reseau">Appliquer…</button>
        <button type="button" id="btn-reseau-actualiser" class="discret-neutre">Actualiser</button></div>
    </form>
    <div id="reseau-confirmation" class="confirmation cachee" role="alertdialog" aria-labelledby="reseau-confirmation-texte">
      <p id="reseau-confirmation-texte"></p>
      <div><button type="button" id="btn-reseau-confirmer">Changer et redémarrer le serveur</button>
        <button type="button" id="btn-reseau-annuler" class="discret-neutre">Annuler</button></div>
    </div>
    <p id="reseau-resultat" class="aide" role="status"></p>
  </section>

  <section class="carte cachee" id="carte-actions">
    <h2>Administration</h2>
    <h3>Sauvegarde</h3>
    <p class="aide">
      Une copie complète, WAL inclus. Automatique toutes les 24 h ;
      ce bouton en fait une tout de suite.
    </p>
    <button id="btn-sauvegarde">Sauvegarder maintenant</button>
    <p id="resultat-sauvegarde" class="aide"></p>
    <h3>Entretien</h3>
    <p class="aide">
      Réaffecte les règlements fournisseur restés globaux, reconstruit
      les index, compacte la base&nbsp;; une copie est écrite avant. À
      faire hors ouverture&nbsp;: pendant l'opération, les caisses
      patientent.
    </p>
    <button id="btn-entretien">Entretenir maintenant</button>
    <p id="resultat-entretien" class="aide"></p>
  </section>

  <section class="carte cachee" id="carte-journal">
    <h2>Journal</h2>
    <p class="aide">
      Les 200 dernières lignes du journal technique, les plus récentes
      en haut&nbsp;: refus, erreurs, erreurs des caisses, commandes lentes,
      démarrages, sauvegardes. Lecture seule.
    </p>
    <div class="filtres" id="niveaux" role="group" aria-label="Niveau">
      <button type="button" class="niveau actif" data-niveau="tout">Tout</button>
      <button type="button" class="niveau" data-niveau="ERREUR">Erreurs</button>
      <button type="button" class="niveau" data-niveau="REFUS">Refus</button>
      <button type="button" class="niveau" data-niveau="AVERT">Avertissements</button>
      <button type="button" class="niveau" data-niveau="POSTE">Caisses</button>
      <button type="button" class="niveau" data-niveau="INFO">Infos</button>
      <button type="button" id="btn-journal" class="discret-neutre">Actualiser</button>
    </div>
    <p id="fichier-journal" class="aide"></p>
    <div id="journal" class="journal"></div>
  </section>
</main>

<footer>
  Cette page est servie par le serveur lui-même. La fermer n'arrête
  rien&nbsp;; fermer la fenêtre noire, si.
</footer>
<script>{script}</script>
</body></html>"#
    )
}

const STYLE: &str = r#"
:root {
  --fond: #f6f8f5; --carte: #fff; --encre: #14181a; --gris: #5e6b65;
  --filet: #d6ded8; --vert: #2f6b4f; --rouge: #a32b21; --ambre: #9a6b00;
}
@media (prefers-color-scheme: dark) {
  :root { --fond:#101413; --carte:#181e1c; --encre:#e8efea; --gris:#93a099;
          --filet:#2a332f; --vert:#5fbf92; --rouge:#e8705f; --ambre:#d9a441; }
}
* { box-sizing: border-box; }
body { margin:0; background:var(--fond); color:var(--encre);
       font:15px/1.5 -apple-system, "Segoe UI", Roboto, sans-serif; }
header { display:flex; align-items:center; gap:12px; padding:16px 20px;
         border-bottom:1px solid var(--filet); }
h1 { font-size:18px; margin:0; }
h2 { font-size:13px; text-transform:uppercase; letter-spacing:.06em;
     color:var(--gris); margin:0 0 10px; }
h3 { font-size:14px; margin:18px 0 6px; }
h3:first-of-type { margin-top:0; }
main { max-width:760px; margin:0 auto; padding:20px; display:grid; gap:16px; }
.carte { background:var(--carte); border:1px solid var(--filet);
         border-radius:8px; padding:16px; }
.cachee { display:none; }
dl { display:grid; grid-template-columns:auto 1fr; gap:6px 16px; margin:0; }
dt { color:var(--gris); }
dd { margin:0; font-weight:600; text-align:right; }
code { background:var(--fond); border:1px solid var(--filet); border-radius:4px;
       padding:6px 10px; font-size:17px; display:inline-block; }
.pastille { font-size:12px; padding:3px 10px; border-radius:20px;
            border:1px solid var(--filet); color:var(--gris); }
.pastille.ok { color:var(--vert); border-color:var(--vert); }
.pastille.ko { color:var(--rouge); border-color:var(--rouge); }
.aide { color:var(--gris); font-size:13px; margin:6px 0; }
.erreur { color:var(--rouge); font-size:13px; min-height:1em; margin:6px 0 0; }
form { display:flex; gap:8px; flex-wrap:wrap; }
input { flex:1; min-width:150px; padding:8px 10px; border-radius:6px;
        border:1px solid var(--filet); background:var(--fond); color:var(--encre); }
button { padding:8px 16px; border-radius:6px; border:1px solid var(--filet);
         background:var(--encre); color:var(--fond); cursor:pointer; }
button.discret { background:transparent; color:var(--rouge);
                 border-color:var(--rouge); padding:4px 10px; font-size:12px; }
table { width:100%; border-collapse:collapse; font-size:14px; }
th { text-align:left; font-size:11px; text-transform:uppercase; color:var(--gris);
     border-bottom:1px solid var(--filet); padding:6px 4px; font-weight:600; }
td { padding:8px 4px; border-bottom:1px solid var(--filet); }
.inactif { color:var(--gris); }
.filtres { display:flex; gap:6px; flex-wrap:wrap; margin:8px 0; }
button.niveau, button.discret-neutre { background:transparent; color:var(--encre);
       padding:4px 10px; font-size:12px; }
button.niveau.actif { background:var(--encre); color:var(--fond); }
button.discret-neutre { margin-left:auto; }
.journal { font:12px/1.45 ui-monospace, Consolas, monospace; max-height:420px;
           overflow:auto; border:1px solid var(--filet); border-radius:6px;
           background:var(--fond); }
.journal div { padding:3px 8px; border-bottom:1px solid var(--filet);
               white-space:pre-wrap; word-break:break-word; }
.journal .ERREUR { color:var(--rouge); }
.journal .AVERT, .journal .POSTE { color:var(--ambre); }
.journal .INFO { color:var(--gris); }
footer { text-align:center; color:var(--gris); font-size:12px; padding:20px; }
.avert { color:var(--ambre); font-size:13px; margin:8px 0; padding:8px 10px;
         border:1px solid var(--ambre); border-radius:6px; }
.a-saisir { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.a-saisir .note { color:var(--gris); font-size:12px; }
.form-reseau { flex-direction:column; align-items:flex-start; gap:10px; }
.form-reseau .champ { display:flex; align-items:center; gap:8px; }
.form-reseau .champ input { flex:none; width:110px; min-width:0; }
fieldset { border:1px solid var(--filet); border-radius:6px; padding:6px 12px; margin:0; }
legend { color:var(--gris); font-size:13px; padding:0 4px; }
.choix { display:flex; align-items:center; gap:8px; padding:4px 0; }
.choix input { flex:none; min-width:0; width:auto; }
button:disabled { opacity:.5; cursor:not-allowed; }
.confirmation { margin-top:12px; padding:12px; border:1px solid var(--ambre);
                border-radius:6px; }
.confirmation p { margin:0 0 10px; }
.confirmation.cachee { display:none; }
"#;

const SCRIPT: &str = r#"
let jeton = null;
let moiPoste = null;

const $ = (id) => document.getElementById(id);

function texteDate(v) {
  if (!v) return "—";
  const d = new Date(v);
  return isNaN(d) ? v : d.toLocaleString("fr-FR");
}

async function rafraichirEtat() {
  try {
    const r = await fetch("/sante");
    const s = await r.json();
    $("pastille").textContent = s.base_saine ? "en service" : "base abîmée";
    $("pastille").className = "pastille " + (s.base_saine ? "ok" : "ko");
    $("etat").innerHTML = `
      <dt>Version</dt><dd>${s.version_serveur}</dd>
      <dt>Protocole</dt><dd>v${s.version_protocole}</dd>
      <dt>Base</dt><dd>${s.base_saine ? "saine" : "ABÎMÉE — restaurer"}</dd>
      <dt>Postes connectés</dt><dd>${s.postes_connectes}</dd>
      <dt>Démarré</dt><dd>${texteDate(s.demarre_le)}</dd>
      <dt>Dernière sauvegarde</dt><dd>${
        s.derniere_sauvegarde ? texteDate(s.derniere_sauvegarde)
                              : "aucune depuis le démarrage"}</dd>`;
    $("adresse").textContent = location.host;
  } catch (e) {
    $("pastille").textContent = "injoignable";
    $("pastille").className = "pastille ko";
  }
}

async function appeler(commande, params) {
  const r = await fetch("/rpc", {
    method: "POST",
    headers: { "Content-Type": "application/json",
               "Authorization": "Bearer " + jeton },
    body: JSON.stringify({ commande, params: params || {} }),
  });
  const c = await r.json();
  if (c.etat === "erreur") throw c.message;
  return c.donnee;
}

$("form-connexion").addEventListener("submit", async (e) => {
  e.preventDefault();
  $("erreur").textContent = "";
  try {
    const r = await fetch("/connexion", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        identifiant: $("identifiant").value.trim(),
        mot_de_passe: $("motdepasse").value,
        poste_nom: "Console du serveur",
        // Empreinte fixe, la meme que console::EMPREINTE : le
        // serveur la reconnait et donne a ce poste le genre
        // « console ».
        poste_empreinte: "console-serveur",
        version_protocole: 1,
      }),
    });
    const c = await r.json();
    if (!r.ok) throw c.message || "Connexion refusée";
    jeton = c.jeton;
    moiPoste = c.poste_id;
    $("carte-identite").classList.add("cachee");
    for (const id of ["carte-postes", "carte-sessions", "carte-reseau", "carte-actions", "carte-journal"])
      $(id).classList.remove("cachee");
    rafraichirSupervision();
    rafraichirJournal();
    rafraichirReseau();
  } catch (err) {
    $("erreur").textContent = String(err);
    $("motdepasse").value = "";
  }
});

async function rafraichirSupervision() {
  if (!jeton) return;
  try {
    const postes = await appeler("lire_postes");
    $("postes").innerHTML = postes.length ? `<table>
      <tr><th>Nom</th><th>Genre</th><th>Dernier contact</th><th></th></tr>
      ${postes.map(p => `<tr class="${p.actif ? "" : "inactif"}">
        <td>${p.nom}${p.actif ? "" : " (désactivé)"}</td>
        <td>${p.genre}</td>
        <td>${texteDate(p.dernier_contact)}</td>
        <td>${p.genre !== "caisse" ? "" :
          `<button class="discret" onclick="basculerPoste('${p.id}', ${p.actif})">
             ${p.actif ? "Désactiver" : "Réactiver"}</button>`}</td>
      </tr>`).join("")}</table>` : "<p class='aide'>Aucun poste inscrit.</p>";

    const sessions = await appeler("lire_sessions_reseau");
    $("sessions").innerHTML = sessions.length ? `<table>
      <tr><th>Poste</th><th>Utilisateur</th><th>Depuis</th><th></th></tr>
      ${sessions.map(s => `<tr>
        <td>${s.poste_nom}</td>
        <td>${s.utilisateur_nom} (${s.role})</td>
        <td>${texteDate(s.ouvert_le)}</td>
        <td>${s.poste_id === moiPoste
          ? "<span class='aide'>cette page</span>"
          : `<button class="discret" onclick="revoquer('${s.id}')">
              Déconnecter</button>`}</td>
      </tr>`).join("")}</table>`
      : "<p class='aide'>Personne n'est connecté.</p>";
  } catch (err) {
    // Session tombée : on redemande les identifiants plutôt que de
    // laisser une page qui ne se met plus à jour sans le dire.
    jeton = null;
    $("carte-identite").classList.remove("cachee");
    $("erreur").textContent = String(err);
    for (const id of ["carte-postes", "carte-sessions", "carte-reseau", "carte-actions", "carte-journal"])
      $(id).classList.add("cachee");
  }
}

async function basculerPoste(id, actif) {
  try {
    // Le serveur nomme ses parametres en serpent : « posteId » lui
    // arrive comme un parametre manquant, pas comme une erreur de nom.
    await appeler(actif ? "desactiver_poste" : "reactiver_poste", { poste_id: id });
    rafraichirSupervision();
  } catch (e) { alert(e); }
}

async function revoquer(id) {
  try {
    await appeler("revoquer_session_reseau", { session_id: id });
    rafraichirSupervision();
  } catch (e) { alert(e); }
}

$("btn-sauvegarde").addEventListener("click", async () => {
  $("resultat-sauvegarde").textContent = "Copie en cours…";
  try {
    const r = await fetch("/sauvegarde", {
      method: "POST",
      headers: { "Authorization": "Bearer " + jeton },
    });
    const c = await r.json();
    $("resultat-sauvegarde").textContent =
      c.fichier ? "Écrite : " + c.fichier : (c.message || "Échec");
  } catch (e) {
    $("resultat-sauvegarde").textContent = String(e);
  }
  rafraichirEtat();
});

$("btn-entretien").addEventListener("click", async () => {
  $("resultat-entretien").textContent = "Entretien en cours — les caisses patientent…";
  try {
    const r = await fetch("/entretien", {
      method: "POST",
      headers: { "Authorization": "Bearer " + jeton },
    });
    const c = await r.json();
    if (c.etat === "ok") {
      const kio = n => (n / 1024).toFixed(0) + " Kio";
      $("resultat-entretien").textContent =
        "Fait : " + c.reimputes + " règlement(s) réaffecté(s), " +
        kio(c.taille_avant) + " → " + kio(c.taille_apres) + ", copie : " + c.copie;
    } else {
      $("resultat-entretien").textContent = c.message || "Échec";
    }
  } catch (e) {
    $("resultat-entretien").textContent = String(e);
  }
  rafraichirEtat();
});

// Le journal : les lignes s'ecrivent en TEXTE (textContent), jamais en
// HTML — un message vient d'une caisse ou d'une requete, il ne doit
// rien pouvoir injecter dans la page du patron.
let niveauJournal = "tout";
async function rafraichirJournal() {
  if (!jeton) return;
  const zone = $("journal");
  try {
    const r = await fetch("/journal?n=200&niveau=" + encodeURIComponent(niveauJournal), {
      headers: { "Authorization": "Bearer " + jeton },
    });
    const c = await r.json();
    if (!r.ok) throw c.message || "Lecture refusée";
    $("fichier-journal").textContent = c.fichier ? "Fichier : " + c.fichier : "Journal sur la console seulement.";
    zone.replaceChildren();
    const lignes = c.lignes.slice().reverse();
    if (!lignes.length) {
      const vide = document.createElement("div");
      vide.className = "INFO";
      vide.textContent = "Rien à ce niveau.";
      zone.append(vide);
    }
    for (const l of lignes) {
      const d = document.createElement("div");
      const m = l.match(/^\S+ \[(\w+)/);
      d.className = m ? m[1] : "";
      d.textContent = l;
      zone.append(d);
    }
  } catch (e) {
    zone.replaceChildren();
    const d = document.createElement("div");
    d.className = "ERREUR";
    d.textContent = String(e);
    zone.append(d);
  }
}
for (const b of document.querySelectorAll("button.niveau")) {
  b.addEventListener("click", () => {
    niveauJournal = b.dataset.niveau;
    for (const x of document.querySelectorAll("button.niveau")) x.classList.toggle("actif", x === b);
    rafraichirJournal();
  });
}
$("btn-journal").addEventListener("click", rafraichirJournal);

// ---- Le reseau du serveur (27/10/2026) ----
// Tout s'ecrit en TEXTE : un nom de carte reseau vient du systeme, il ne
// doit rien pouvoir injecter dans la page.
let reseau = null;
let demandeReseau = null;

function etatRegle(v) {
  return v === true ? "ouverte" : v === false ? "FERMÉE" : "inconnue";
}

function ajouterLigne(dl, terme, valeur) {
  const dt = document.createElement("dt");
  dt.textContent = terme;
  const dd = document.createElement("dd");
  dd.textContent = valeur;
  dl.append(dt, dd);
}

function afficherReseau(c) {
  const dl = $("reseau-etat");
  dl.replaceChildren();
  ajouterLigne(dl, "Nom du poste", c.poste || "—");
  if (!c.cartes.length) ajouterLigne(dl, "Adresse IP", "aucune : ce poste n'est joignable que par lui-même");
  for (const k of c.cartes) {
    ajouterLigne(dl, "Adresse IP" + (k.carte ? " (" + k.carte + ")" : ""),
      k.ip + (k.dhcp ? " · donnée par la box" : " · fixe") + (k.principale ? " · principale" : ""));
  }
  ajouterLigne(dl, "Port", String(c.port));
  ajouterLigne(dl, "Écoute", c.ecoute + " — " + c.ecoute_libelle);
  ajouterLigne(dl, "Pare-feu, caisses (TCP " + c.plage[0] + "-" + c.plage[1] + ")", etatRegle(c.parefeu.tcp));
  ajouterLigne(dl, "Pare-feu, recherche (UDP " + c.decouverte.port + ")", etatRegle(c.parefeu.udp));
  ajouterLigne(dl, "Recherche automatique", c.decouverte.active
    ? "en service (UDP " + c.decouverte.port + ")" : "arrêtée — " + (c.decouverte.raison || ""));
  ajouterLigne(dl, "Lancé", c.en_service ? "en service Windows" : "à la main");
  if (c.configuration) ajouterLigne(dl, "Réglage", c.configuration);

  const av = $("reseau-avertissements");
  av.replaceChildren();
  const avertir = (t) => {
    const p = document.createElement("p");
    p.className = "avert";
    p.textContent = t;
    av.append(p);
  };
  if (c.avertissement_dhcp) avertir(c.avertissement_dhcp);
  if (c.parefeu.tcp === false) {
    avertir("Le pare-feu de Windows n'a pas de règle « Gescom serveur » pour le port " + c.port +
      " : les caisses recevront « Serveur injoignable ». Réinstaller Gescom Serveur, ou, en administrateur : " +
      "outils\\parefeu.ps1 -Ouvrir.");
  }
  if (c.parefeu.udp === false) {
    avertir("La recherche automatique (UDP " + c.decouverte.port + ") est fermée par le pare-feu : " +
      "« Rechercher le serveur » ne trouvera pas ce poste depuis les caisses.");
  }

  const z = $("reseau-a-saisir");
  z.replaceChildren();
  if (!c.a_saisir.length) {
    const p = document.createElement("p");
    p.className = "aide";
    p.textContent = "Le serveur n'écoute que ce poste : aucune caisse ne peut le joindre.";
    z.append(p);
  }
  c.a_saisir.forEach((a, i) => {
    const code = document.createElement("code");
    code.textContent = a;
    z.append(code);
    if (i === 0 && c.a_saisir.length > 1) {
      const n = document.createElement("span");
      n.className = "note";
      n.textContent = "conseillé ;";
      z.append(n);
    }
  });

  $("reseau-port").value = c.port;
  for (const r of document.querySelectorAll('input[name="ecoute"]')) r.checked = r.value === c.ecoute;
  for (const el of $("form-reseau").elements) {
    if (el.id !== "btn-reseau-actualiser") el.disabled = !c.modifiable;
  }
  $("reseau-pourquoi").textContent = c.pourquoi_pas ||
    "Port de " + c.plage[0] + " à " + c.plage[1] + ". Le serveur redémarre sur le nouveau réglage ; " +
    "les caisses devront le retrouver.";
}

async function rafraichirReseau() {
  if (!jeton) return;
  try {
    const r = await fetch("/reseau", { headers: { "Authorization": "Bearer " + jeton } });
    const c = await r.json();
    if (!r.ok) throw c.message || "Lecture refusée";
    reseau = c;
    afficherReseau(c);
  } catch (e) {
    $("reseau-resultat").textContent = String(e);
  }
}

$("btn-reseau-actualiser").addEventListener("click", () => {
  $("reseau-resultat").textContent = "";
  rafraichirReseau();
});

$("form-reseau").addEventListener("submit", (e) => {
  e.preventDefault();
  if (!reseau) return;
  $("reseau-resultat").textContent = "";
  const port = Number($("reseau-port").value);
  const choix = document.querySelector('input[name="ecoute"]:checked');
  const ecoute = choix ? choix.value : reseau.ecoute;
  if (!Number.isInteger(port) || port < reseau.plage[0] || port > reseau.plage[1]) {
    $("reseau-resultat").textContent = "Choisir un port de " + reseau.plage[0] + " à " + reseau.plage[1] + ".";
    return;
  }
  if (port === reseau.port && ecoute === reseau.ecoute) {
    $("reseau-resultat").textContent = "Rien ne change.";
    return;
  }
  const n = reseau.postes_connectes;
  const qui = n === 0 ? "Aucun autre poste n'est connecté en ce moment."
    : n === 1 ? "1 autre poste est connecté : il perdra le serveur."
    : n + " autres postes sont connectés : ils perdront le serveur.";
  $("reseau-confirmation-texte").textContent = qui +
    " Le serveur va redémarrer sur le port " + port +
    (ecoute === "127.0.0.1" ? ", pour ce poste seulement : aucune caisse ne le joindra plus." : ".") +
    " Pendant le redémarrage" + (reseau.en_service ? " (quelques secondes, jusqu'à une minute)" : "") +
    ", personne ne peut vendre. Ensuite, sur chaque caisse : Paramètres → Réseau → « Rechercher le serveur » " +
    "(ou taper la nouvelle adresse).";
  demandeReseau = { port, ecoute };
  $("reseau-confirmation").classList.remove("cachee");
  $("btn-reseau-confirmer").focus();
});

$("btn-reseau-annuler").addEventListener("click", () => {
  demandeReseau = null;
  $("reseau-confirmation").classList.add("cachee");
});

$("btn-reseau-confirmer").addEventListener("click", async () => {
  if (!demandeReseau) return;
  $("btn-reseau-confirmer").disabled = true;
  $("reseau-resultat").textContent = "Enregistrement…";
  try {
    const r = await fetch("/reseau", {
      method: "POST",
      headers: { "Content-Type": "application/json", "Authorization": "Bearer " + jeton },
      body: JSON.stringify(demandeReseau),
    });
    const c = await r.json();
    if (!r.ok || c.etat !== "ok") throw c.message || "Changement refusé";
    $("reseau-confirmation").classList.add("cachee");
    $("reseau-resultat").textContent = "Enregistré. Le serveur redémarre sur le port " + c.port +
      "… cette page se rouvrira toute seule.";
    attendreEtRouvrir(c.console, c.attente_max_s || 60);
  } catch (e) {
    $("reseau-resultat").textContent = String(e);
    $("btn-reseau-confirmer").disabled = false;
  }
});

// La console se rouvre sur le nouveau port des que le serveur y repond.
async function attendreEtRouvrir(adresse, max) {
  const sante = adresse.replace(/\/console$/, "/sante");
  const debut = Date.now();
  while (Date.now() - debut < max * 1000) {
    await new Promise((ok) => setTimeout(ok, 1500));
    try {
      const r = await fetch(sante, { cache: "no-store" });
      if (r.ok) { location.href = adresse; return; }
    } catch (e) { /* pas encore */ }
  }
  $("reseau-resultat").textContent = "Le serveur ne répond pas encore à " + adresse +
    ". Regarder son journal, ou relancer Gescom Serveur.";
}

rafraichirEtat();
// Dix secondes : assez pour voir une caisse se connecter, assez peu
// pour ne pas transformer la page en source de charge.
setInterval(rafraichirEtat, 10000);
setInterval(rafraichirSupervision, 10000);
"#;
