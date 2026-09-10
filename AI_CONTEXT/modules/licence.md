# Module : licence

Rôle : fermer la portabilité. En v1, `Gescom.exe` se suffisait à
lui-même — copié sur une clé USB, il démarrait ailleurs, sans qu'aucune
ligne de code ne vérifie quoi que ce soit. Une vente valait un nombre
illimité d'installations.

## Fichiers

| Fichier | Rôle |
|---|---|
| [noyau/src/empreinte.rs](../../src-tauri/noyau/src/empreinte.rs) | le code du poste |
| [noyau/src/licence.rs](../../src-tauri/noyau/src/licence.rs) | vérification, essai |
| [commandes/licence.rs](../../src-tauri/src/commandes/licence.rs) | 4 commandes Tauri |
| [serveur/src/main.rs](../../src-tauri/serveur/src/main.rs) | `lire_licence` au démarrage |
| [serveur/src/api.rs](../../src-tauri/serveur/src/api.rs) | plafond appliqué à `/connexion` |
| [pages/PageLicence.tsx](../../src/pages/PageLicence.tsx) | l'écran qui barre la route |
| [outils/licence/](../../src-tauri/outils/licence/) | ⚠️ outil de l'ÉDITEUR |

## Le jeton

`GESCOM1.<base64url(json)>.<base64url(signature Ed25519)>`

Le JSON porte `boutique`, `nif`, `empreinte`, `postes_max`, `emis_le`,
`expire_le`. La signature couvre les **octets décodés**, pas la chaîne
base64 — sinon deux encodages du même contenu passeraient, dont un qu'on
n'a pas relu.

## Règles métier

- [CONFIRMÉ] L'exécutable ne contient que la clef **publique**
  ([licence.rs:46](../../src-tauri/noyau/src/licence.rs#L46)). Le
  désosser ne donne pas de quoi émettre. Avec un secret partagé, le
  premier qui l'extrait publie un générateur et la protection tombe
  partout d'un coup.
- [CONFIRMÉ] L'empreinte est le `MachineGuid` de Windows, salé et
  condensé en 12 caractères hexadécimaux — `B0E1-7DF5-9524`. **Rien
  d'autre** : y ajouter le nom de machine ou le numéro de volume
  casserait la licence le jour d'un renommage ou d'un changement de
  disque, et fabriquerait un appel un samedi de marché.
- [CONFIRMÉ] Une réinstallation de Windows **change** l'empreinte et
  invalide la licence. Assumé : l'éditeur réémet, et c'est ce qui rend
  la copie visible.
- [CONFIRMÉ] La comparaison d'empreinte ignore tirets et casse : le code
  se dicte au téléphone et se retape (test
  `l_empreinte_se_compare_sans_tirets_ni_casse`).
- [CONFIRMÉ] Espaces et retours à la ligne sont retirés avant lecture —
  les licences voyagent par WhatsApp (test
  `les_espaces_de_whatsapp_ne_genent_pas`).
- [CONFIRMÉ] Le **dernier jour** d'une licence est encore valable : une
  expiration en avance d'un jour bloque une caisse un matin, sans que
  personne ne comprenne (test `le_dernier_jour_est_encore_bon`).
- [CONFIRMÉ] Modifier `postes_max` dans le fichier casse la signature
  (test `un_contenu_modifie_casse_la_signature`).
- [CONFIRMÉ] `activer_licence` n'écrit **que** si le jeton est valable
  ici : enregistrer un jeton refusé remplacerait une licence qui marche
  par une qui ne marche pas
  ([commandes/licence.rs](../../src-tauri/src/commandes/licence.rs)).
- [CONFIRMÉ] Une licence expirée ou d'un autre poste ne **retombe pas**
  sur l'essai : le message perdrait sa cause.
- [CONFIRMÉ] L'essai dure 30 jours, témoin écrit à **deux** endroits —
  `config_app.premiere_ouverture` et
  `HKCU\Software\Gescom\PremiereOuverture` ; la plus ancienne gagne.
  Effacer la base ne rend pas trente jours neufs. Ce n'est **pas**
  inviolable : les deux se nettoient, c'est un ralentisseur honnête.
- [CONFIRMÉ] L'essai vaut **un** poste (test
  `l_essai_ne_vaut_quun_poste`) : sinon trente jours suffiraient à
  équiper un magasin.
- [CONFIRMÉ] En mode **poste caisse**, le client ne vérifie rien : la
  licence vit sur le serveur, qui compte les postes à `/connexion`.
  Sans quoi un client achetant trois postes devrait activer trois
  machines, et ses caisses s'arrêteraient au bout de trente jours
  ([App.tsx](../../src/App.tsx)).
- [CONFIRMÉ] Le serveur compte les postes **inscrits et actifs**, pas
  les connectés : sinon dix machines travailleraient à tour de rôle sur
  une licence de trois. Le poste déjà inscrit passe toujours — dépasser
  le plafond une fois ne doit pas condamner ceux qui travaillaient
  avant ([api.rs](../../src-tauri/serveur/src/api.rs)).
- [CONFIRMÉ] Sans licence valable, le serveur **démarre quand même**,
  avec un avertissement : un service qui refuse de démarrer est une
  boutique qui n'ouvre pas sans que personne ne sache pourquoi. Ce sont
  les connexions de caisse qui sont refusées, avec le motif.
- [CONFIRMÉ] Le bandeau d'essai n'apparaît que dans les **7 derniers
  jours**. Plus tôt, c'est du harcèlement commercial dans un logiciel de
  comptoir.

## L'outil de l'éditeur

```bash
gescom-licence clef                      # une fois, au tout début
gescom-licence empreinte                 # le code de ce poste
gescom-licence signer --boutique "…" --empreinte B0E1-7DF5-9524 \
                      --postes 3 [--expire AAAA-MM-JJ] --sortie x.licence
gescom-licence verifier --fichier x.licence [--empreinte CODE]
```

⚠️ **Ne se livre jamais au client** — il contient de quoi émettre.

La **clef privée** vit dans `%USERPROFILE%\.gescom\licence_privee.key`,
hors du dépôt. `.gitignore` bloque `*.key`, `licence_privee*` et
`*.licence` en seconde barrière. La perdre = ne plus pouvoir émettre une
seule licence, ni réémettre à qui a réinstallé Windows. **À sauvegarder
comme la base d'un client.**

## Ce que ça ne fait pas

Ça n'arrête pas quelqu'un qui démonte l'exécutable et retire le test —
aucune protection logicielle ne le fait. Ce qui est empêché, c'est la
copie **ordinaire** : le client qui installe dans sa deuxième boutique,
le revendeur qui duplique sans le dire. C'est là qu'est l'argent perdu.

La base reste du SQLite **non chiffré** : la licence protège l'éditeur,
pas les données du commerçant. Chantier distinct (SQLCipher), écarté
pour l'instant.

## Tests

16 tests dans `licence.rs` et `empreinte.rs` — chemin nominal signé avec
une clef de test à graine fixe, empreinte tolérante, licence flottante,
expiration au jour près, contenu falsifié, signature d'un autre
émetteur, bruit de WhatsApp, essai. Plus une vérification bout en bout
faite avec la vraie clef : émission, acceptation, refus sur un autre
poste, refus après passage de 3 à 99 postes dans le fichier.
