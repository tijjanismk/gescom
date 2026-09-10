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
//! ouvre `http://localhost:7300` sur le poste principal — ou l'adresse
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
      Sur le poste caisse : <b>Paramètres → Réseau → Poste caisse</b>,
      puis cette adresse.
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

  <section class="carte cachee" id="carte-actions">
    <h2>Sauvegarde</h2>
    <p class="aide">
      Une copie complète, WAL inclus. Automatique toutes les 24 h ;
      ce bouton en fait une tout de suite.
    </p>
    <button id="btn-sauvegarde">Sauvegarder maintenant</button>
    <p id="resultat-sauvegarde" class="aide"></p>
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
footer { text-align:center; color:var(--gris); font-size:12px; padding:20px; }
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
    for (const id of ["carte-postes", "carte-sessions", "carte-actions"])
      $(id).classList.remove("cachee");
    rafraichirSupervision();
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
    for (const id of ["carte-postes", "carte-sessions", "carte-actions"])
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

rafraichirEtat();
// Dix secondes : assez pour voir une caisse se connecter, assez peu
// pour ne pas transformer la page en source de charge.
setInterval(rafraichirEtat, 10000);
setInterval(rafraichirSupervision, 10000);
"#;
