// La page locale de Gescom Serveur (D25).
//
// Elle demande a la fenetre de trouver le serveur de ce poste : s'il
// repond, la fenetre affiche sa console et cette page disparait. Sinon
// elle dit pourquoi, propose de demarrer le service, et reessaie toute
// seule toutes les cinq secondes.

const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);
let minuterie = null;
let occupe = false;

function message(texte, genre) {
  $("message").textContent = texte || "";
  $("message").className = "message" + (genre ? " " + genre : "");
}

const SERVICE = {
  absent: "Le service « Gescom Serveur » n'est pas installé sur ce poste. Installer Gescom Serveur " +
    "(son installeur, en administrateur) — ou, si ce poste garde la base sans service, ouvrir Gescom : " +
    "il lance son serveur lui-même.",
  arrete: "Le service « Gescom Serveur » est arrêté. Le démarrer : Windows demandera l'accord d'un administrateur.",
  en_cours: "Le service « Gescom Serveur » démarre ou s'arrête : patienter quelques secondes.",
  en_marche: "Le service « Gescom Serveur » tourne, mais ne répond pas encore : il ouvre peut-être la base. " +
    "S'il ne répond toujours pas dans une minute, son journal dit pourquoi (%ProgramData%\\Gescom\\serveur.log).",
  inconnu: "L'état du service « Gescom Serveur » n'a pas pu être lu.",
};

function planifier() {
  clearTimeout(minuterie);
  minuterie = setTimeout(chercher, 5000);
}

async function chercher() {
  if (occupe) return;
  if (!invoke) {
    message("Cette page ne s'ouvre que dans la fenêtre Gescom Serveur.", "erreur");
    return;
  }
  occupe = true;
  try {
    const e = await invoke("chercher_serveur");
    if (e.trouve) {
      // La fenetre s'en va vers la console.
      $("titre").textContent = "Ouverture de la console…";
      return;
    }
    $("titre").textContent = "Le serveur ne tourne pas";
    $("detail").textContent = "Personne ne répond à l'adresse " + e.adresse + " de ce poste." +
      (e.hors_plage ? " Ce port n'est pas dans la plage 40000-40100 : les caisses ne le joindraient pas." : "");
    $("service").textContent = SERVICE[e.service] || SERVICE.inconnu;
    $("btn-demarrer").hidden = !(e.service === "arrete" || e.service === "inconnu");
    $("btn-reessayer").hidden = false;
    $("configuration").textContent = "Réglage lu : " + e.configuration;
    planifier();
  } catch (err) {
    message(String(err), "erreur");
    $("btn-reessayer").hidden = false;
    planifier();
  } finally {
    occupe = false;
  }
}

$("btn-reessayer").addEventListener("click", () => {
  message("");
  clearTimeout(minuterie);
  chercher();
});

$("btn-demarrer").addEventListener("click", async () => {
  clearTimeout(minuterie);
  const bouton = $("btn-demarrer");
  bouton.disabled = true;
  message("Windows va demander l'accord d'un administrateur…");
  try {
    const m = await invoke("demarrer_service");
    message(m + " Recherche du serveur…", "ok");
    // Le serveur ouvre sa base : on lui laisse un instant.
    setTimeout(chercher, 2000);
  } catch (err) {
    message(String(err), "erreur");
    planifier();
  } finally {
    bouton.disabled = false;
  }
});

if (new URLSearchParams(location.search).get("perdu")) {
  message("La console ne répondait plus : le serveur s'est peut-être arrêté.", "erreur");
}
chercher();
