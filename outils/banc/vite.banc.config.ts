// La config Vite du projet, plus une chose pour le banc : ne pas
// surveiller outils/banc/.travail. Le profil WebView2 de la fenetre
// monoposte y vit, ses fichiers sont verrouilles, et le surveillant de
// Vite tombait dessus (EBUSY) en emportant l'ecran.
import { mergeConfig } from "vite";
import projet from "../../vite.config";

export default mergeConfig(projet, {
  server: { watch: { ignored: ["**/outils/banc/.travail/**"] } },
});
