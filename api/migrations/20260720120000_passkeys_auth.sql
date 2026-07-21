-- Authentification par passkeys WebAuthn et appareils enrôlés (cf. ADR-0006).
-- Remplace pseudo + mot de passe + lien d'invitation (ADR-0002).

-- --- Fenêtre d'enrôlement, portée par le foyer -----------------------------
-- Ouverte au CLI (`weekmeals device open-window`), elle autorise un appareil
-- inconnu à enrôler une passkey tant que `now() < onboarding_until`. Protégée
-- par un code d'appairage à usage unique (seul le hash est stocké) ; au-delà de
-- cinq échecs la fenêtre se referme. `onboarding_user_id` non nul rattache
-- l'enrôlement à un utilisateur existant (`--for`), sinon un nouvel utilisateur
-- est créé à la fin de la cérémonie. En base plutôt qu'en variable d'env : pas
-- de redéploiement pour ajouter un téléphone, et impossible de l'oublier ouverte.
alter table households
    add column onboarding_until     timestamptz,
    add column onboarding_code_hash text,
    add column onboarding_attempts  integer not null default 0,
    add column onboarding_user_id   uuid references users (id) on delete set null;

-- --- Les utilisateurs perdent le mot de passe -------------------------------
-- Plus aucun secret côté serveur : `username` n'est qu'un libellé d'affichage,
-- donc son unicité globale (héritée du login pseudo+mdp) n'a plus lieu d'être.
alter table users drop column password_hash;
alter table users drop constraint users_username_key;

-- --- Appareils enrôlés : une passkey par ligne ------------------------------
-- Le serveur ne stocke que la clé publique. `passkey` est la sérialisation
-- JSON du credential webauthn-rs (clé publique COSE, compteur de signature,
-- AAGUID, drapeaux de sauvegarde) : format opaque, relu tel quel aux cérémonies
-- d'authentification. `credential_id` en est extrait pour l'unicité et les
-- recherches. Les drapeaux `backup_*` sont dénormalisés pour l'affichage.
create table devices (
    id              uuid primary key default gen_random_uuid(),
    user_id         uuid not null references users (id) on delete cascade,
    credential_id   bytea not null unique,
    passkey         jsonb not null,
    label           text not null,
    backup_eligible boolean not null default false,
    backup_state    boolean not null default false,
    created_at      timestamptz not null default now(),
    last_seen_at    timestamptz
);

create index devices_user_id_idx on devices (user_id);

-- --- Le lien d'invitation disparaît (ADR-0006) ------------------------------
-- Remplacé par le code d'appairage lu sur un terminal, qui ne transite pas par
-- une messagerie. `BOOTSTRAP_INVITE_CODE` est retiré côté configuration.
drop table invitations;
