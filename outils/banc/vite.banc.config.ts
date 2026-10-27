// La config Vite du projet, plus une chose pour le banc : ne pas
// surveiller outils/banc/.travail. Le profil WebView2 de la fenetre
// monoposte y vit, ses fichiers sont verrouilles, et le surveillant de
// Vite tombait dessus (EBUSY) en emportant l'ecran.
import { mergeConfig } from "vite";
import projet from "../../vite.config";

// Et sans rechargement a chaud : le banc ne modifie pas le code pendant
// qu'il tourne, et quand la liaison HMR se coupe le client Vite ouvre un
// SharedWorker (pour retrouver le serveur) que Playwright ne sait pas
// rattacher par WebView2 (« shared_worker », pid 0) : la connexion a la
// fenetre plantait.
export default mergeConfig(projet, {
  server: { hmr: false, watch: { ignored: ["**/outils/banc/.travail/**"] } },
});
