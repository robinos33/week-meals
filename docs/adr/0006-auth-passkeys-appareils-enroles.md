# ADR-0006 — Authentification par passkeys et appareils enrôlés

- **Statut :** proposée (2026-07-20)
- **Remplace :** [ADR-0002](0002-auth-sans-email.md) (authentification sans email)

## Contexte

L'ADR-0002 actait un login **pseudo + mot de passe** avec lien d'invitation.
À l'usage ce modèle ne correspond pas au besoin réel : deux personnes, deux
téléphones, une liste de courses consultée en trente secondes au supermarché.
Saisir un mot de passe dans ce contexte est une friction disproportionnée, et
un mot de passe partagé dans un couple finit invariablement en post-it.

Le besoin exprimé est : **que les deux téléphones soient reconnus d'office**,
sans étape de connexion, avec une phase d'enrôlement initiale pour les
désigner, et un contournement en dev pour ne pas s'imposer ce circuit à chaque
`cargo run`.

Une piste envisagée était le **fingerprint navigateur** (canvas, polices, UA,
résolution). Elle est écartée pour deux raisons dirimantes :

- **Ce n'est pas un secret.** Tout ce qui compose une empreinte est diffusé par
  le navigateur à n'importe quel site. C'est une observation de l'appareil, pas
  une preuve de possession : elle se rejoue trivialement.
- **Ce n'est pas stable.** Une mise à jour d'OS ou de navigateur suffit à la
  faire dériver, et Safari/iOS déploient activement de l'anti-fingerprinting.
  Le mode de défaillance est le pire possible : verrouillage hors de sa propre
  app, sans recours depuis le téléphone.

Il faut donc un **secret détenu par l'appareil**, pas une description de
l'appareil.

## Options considérées

1. **Passkeys (WebAuthn), credentials découvrables** ✅
   Paire de clés générée et scellée dans la Secure Enclave du téléphone,
   déverrouillée par Face ID / Touch ID. Le serveur ne stocke que la clé
   publique — aucun secret côté serveur ne peut fuiter.
2. **Jeton d'appareil en cookie `HttpOnly`**
   Jeton aléatoire posé à l'enrôlement, hash stocké en base. Plus simple (~une
   table et une migration), zéro dépendance. Écarté : le cookie est un porteur
   — sa copie suffit à usurper l'appareil — et il ne survit ni à un effacement
   de données de navigation ni à un changement de téléphone.
3. **Fingerprint navigateur** — écarté, voir Contexte.
4. **Statu quo ADR-0002 (pseudo + mot de passe)** — écarté : friction
   quotidienne, et un secret mémorisé pour un usage domestique dérive vers un
   mot de passe faible et partagé.

## Décision

### Le credential : passkey découvrable, vérification utilisateur exigée

Via la crate **`webauthn-rs`**. Deux paramètres de cérémonie portent tout
l'intérêt du choix :

- **Credentials découvrables** (*resident keys*, `ResidentKey::Required`) : le
  téléphone sait à lui seul quelle identité il porte. L'écran d'accueil propose
  directement « Continuer avec Face ID » — **aucun identifiant à saisir**,
  ce qui est exactement l'UX visée.
- **`UserVerification::Required`** : la clé ne s'utilise qu'après Face ID /
  Touch ID / code. Un téléphone volé déverrouillé ne suffit pas.

Après une cérémonie réussie, on ouvre une **session `tower-sessions`** classique
(le cookie `HttpOnly` existant) : Face ID n'est redemandé qu'à l'expiration de
la session, pas à chaque requête.

### Les deux modes d'authentification, dans `Config`

`AUTH_DISABLED` est aujourd'hui lu par un `LazyLock` interne à
`auth::presentation` — global caché, figé pour tout le process, donc peu
testable. Il est remonté dans le `Config` de la crate `server`, aux côtés de
`web_origin` et des réglages de cookie :

```rust
pub enum AuthMode {
    /// Dev : aucune identification, tout est scopé au foyer de démo.
    Disabled,
    /// Prod : seuls les appareils enrôlés passent.
    Locked,
}
```

Lu depuis `AUTH_MODE`, **`Locked` par défaut** si la variable est absente ou
non reconnue : on échoue fermé. Le `.env` local porte `AUTH_MODE=disabled`.

### La fenêtre d'enrôlement : en base, pas en environnement

L'enrôlement n'est **pas** un troisième mode de configuration. En faire une
variable d'environnement imposerait un redéploiement pour ajouter un téléphone,
et surtout laisserait la porte ouverte en cas d'oubli.

C'est un **état en base avec expiration** — sur `households`, les colonnes
`onboarding_until timestamptz null`, `onboarding_code_hash text null` et
`onboarding_attempts int` — piloté par le CLI existant :

```
weekmeals device open-window --minutes 15 [--for <user>]
weekmeals device close-window
weekmeals device list
weekmeals device revoke <id>
```

Tant que `now() < onboarding_until`, un appareil inconnu peut exécuter une
cérémonie **d'enregistrement**. Passé ce délai la fenêtre se referme d'elle-même
et un appareil inconnu reçoit un `401`. Ouvrir la fenêtre suppose un accès shell
au serveur : c'est la racine de confiance du système.

### Le code d'appairage

Une fenêtre temporelle seule laisse un trou : pendant ces quinze minutes,
n'importe qui connaissant l'URL peut s'enrôler. La fenêtre étant courte et
ouverte à la demande, le risque est faible — mais il se ferme pour presque rien.

`open-window` **imprime un code à usage unique** (8 caractères, alphabet sans
ambiguïté visuelle), que le téléphone doit saisir pour enrôler sa passkey :

```
$ weekmeals device open-window --minutes 15
Fenêtre d'enrôlement ouverte jusqu'à 21:47 (15 min).
Code d'appairage :  K7M4-P2QX
```

Seul le **hash** du code est stocké, et les tentatives sont comptées : au-delà
de cinq échecs la fenêtre se referme immédiatement. C'est le seul moment où
quelqu'un tape quelque chose dans cette application — une fois par téléphone,
jamais ensuite.

C'est aussi ce qui remplace le lien d'invitation de l'ADR-0002, sous une forme
plus sûre : un code court-vivant lu sur un terminal, qui ne transite pas par une
messagerie et ne peut pas être transféré par inadvertance.

### Le déroulé de l'enrôlement

1. **Ouverture** — `open-window` sur le serveur ; par défaut la fenêtre crée un
   nouvel utilisateur, `--for <user>` rattache l'appareil à une personne
   existante (deuxième téléphone, tablette).
2. **Le téléphone ouvre l'app** — sans appareil connu, le front interroge
   `GET /auth/enroll/status`. Fenêtre fermée : écran « Cette application est
   verrouillée ». Fenêtre ouverte : écran d'enrôlement (code d'appairage +
   libellé de l'appareil, « iPhone de Robin »).
3. **`POST /auth/enroll/start`** — le serveur vérifie fenêtre et code, appelle
   `start_passkey_registration`, renvoie le challenge. L'état de cérémonie
   (`PasskeyRegistration`) est conservé **en session côté serveur**, jamais
   confié au client.
4. **Cérémonie navigateur** — `navigator.credentials.create()`. Le téléphone
   demande Face ID / empreinte et scelle la clé privée. Côté front on passe par
   `@simplewebauthn/browser` plutôt que de coder à la main les encodages
   base64url des champs binaires.
5. **`POST /auth/enroll/finish`** — `finish_passkey_registration` valide
   l'attestation ; on insère la ligne `devices` et on **ouvre directement la
   session**. L'utilisateur est dans l'app, sans écran de connexion
   supplémentaire.
6. **Fermeture** — par expiration, par `close-window`, ou immédiatement après
   cinq échecs de code.

Le **handle utilisateur** WebAuthn (`user.id`) est l'UUID du `users`, opaque et
stable. Il est stocké sur le téléphone et renvoyé à chaque authentification :
c'est précisément ce qui permet au serveur d'identifier la personne sans qu'elle
saisisse le moindre identifiant. Il ne doit donc contenir aucune donnée
personnelle — le libellé et le pseudo vivent dans `user.name` / `displayName`,
qui ne servent qu'à l'affichage du sélecteur de passkey.

### Le stockage

Une table `devices` : `user_id`, `credential_id` (unique), clé publique COSE,
compteur de signature, AAGUID, drapeaux `backup_eligible` / `backup_state`,
libellé (« iPhone de Robin »), `created_at`, `last_seen_at`. `webauthn-rs`
détecte les régressions de compteur (signal de clonage). Aucun plafond codé en
dur sur le nombre d'appareils — la fenêtre est le contrôle, la révocation le
remède.

La table `users` se réduit à un identifiant, un pseudo d'affichage et le foyer :
plus de hash de mot de passe, plus rien de personnel.

### Ce qui disparaît

Passkeys et mots de passe ne cohabitent pas : garder un login mot de passe
en secours annulerait le bénéfice de sécurité (l'attaquant vise le maillon
faible). Sont donc retirés : les routes login/logout par mot de passe, le
hachage Argon2id, `BOOTSTRAP_INVITE_CODE`, la table `invitations` (via une
migration `drop table`, l'existante étant déjà appliquée).

### Récupération

Il n'y a plus de mot de passe à perdre. Le seul scénario de perte totale est
celui des deux téléphones simultanément inaccessibles : la sortie est
`weekmeals device open-window` sur le serveur — le même recours CLI que
prévoyait l'ADR-0002 pour le reset de mot de passe.

## Conséquences

- **UX conforme à la cible** : ouvrir l'app, Face ID si la session a expiré,
  rien à taper. Jamais d'identifiant, jamais de mot de passe.
- **Plus aucun secret réutilisable côté serveur.** Une fuite de la base ne livre
  que des clés publiques — inexploitables pour se connecter.
- **Les passkeys se synchronisent avec le compte de l'écosystème.** Une passkey
  Apple n'est pas strictement liée *à un téléphone* mais au compte iCloud, et
  une passkey Android se synchronise de la même façon via le gestionnaire de
  mots de passe Google (ou tout fournisseur tiers, Android 14+ ouvrant l'API
  Credential Manager). Un téléphone remplacé et restauré conserve l'accès dans
  les deux cas. C'est un bénéfice, mais l'énoncé exact du modèle de sécurité est
  « les **comptes** enrôlés » (iCloud ou Google), pas « les appareils enrôlés ».
  Les drapeaux `backup_eligible` / `backup_state` de la cérémonie permettent de
  le constater et de l'afficher dans la carte Appareils.
- **Contrainte de domaine** : le `rpId` WebAuthn doit être un domaine enregistré
  et servi en HTTPS. `localhost` est admis en dev comme cas particulier, et de
  toute façon `AuthMode::Disabled` court-circuite tout en local. Corollaire : le
  front et l'API doivent partager le même domaine parent en prod.
- **Écran Paramètres** : la carte « Foyer » (dont le bouton d'invitation mort a
  été retiré) devient une carte **Appareils** — libellé, dernière activité,
  révocation.
- **Coût** : une dépendance (`webauthn-rs`), deux cérémonies à implémenter côté
  front (`navigator.credentials.create` / `.get`), et un état serveur de
  cérémonie en cours à conserver en session. Nettement plus lourd que l'option 2
  du jeton d'appareil ; c'est le prix assumé de la robustesse.
- **Indépendant de l'écosystème.** WebAuthn est un standard W3C implémenté par
  Safari/iOS comme par Chrome/Android : le serveur ne stocke que des clés
  publiques et ignore tout de la plateforme d'en face. Un foyer mixte
  iPhone + Android ne demande aucune configuration particulière — chacun enrôle
  sa passkey, déverrouillée par Face ID d'un côté, empreinte ou déverrouillage
  d'écran de l'autre.
  - **Nuance Android** : la synchronisation suppose un compte Google et un
    verrouillage d'écran actif. Sans cela, la passkey créée reste liée au
    matériel — parfaitement fonctionnelle, mais perdre le téléphone impose un
    ré-enrôlement (`weekmeals device open-window`).
  - **Nuance PWA** : rien de spécial tant qu'on reste dans le navigateur. Si
    l'app était un jour empaquetée en TWA sur le Play Store, il faudrait publier
    un `assetlinks.json` pour associer l'application au domaine.
- **Connexion inter-appareils gratuite** : le flux hybride (QR + Bluetooth) du
  standard permet de se connecter sur un ordinateur en scannant avec un
  téléphone déjà enrôlé, et ce **entre écosystèmes** (téléphone Android,
  navigateur macOS et inversement). Cela couvre l'usage « dimanche sur le
  canapé » depuis un laptop sans avoir à enrôler le laptop.
- **Pas de dégradation possible** : un navigateur sans WebAuthn ne peut pas se
  connecter du tout. Acceptable pour un parc de deux téléphones connus et
  récents ; sur Android cela suppose Android 9+ avec les services Google Play.
