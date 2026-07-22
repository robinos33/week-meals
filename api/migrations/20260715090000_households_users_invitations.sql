-- Socle multi-foyers + auth sans email (cf. ADR-0002).
-- Toutes les données applicatives sont scopées à un household.
--
-- Types SQLite (cf. ADR-0008) : les UUID sont des `blob` de 16 octets
-- (encodage natif SQLx) et les horodatages du texte. `datetime('now')` produit
-- de l'UTC, comme `now()` le faisait en `timestamptz`.

create table households (
    id         blob primary key,
    name       text not null,
    created_at text not null default (datetime('now'))
);

-- Login pseudo + mot de passe (Argon2id). username unique globalement :
-- il identifie l'utilisateur au login sans sélection de foyer ni email.
create table users (
    id            blob primary key,
    household_id  blob not null references households (id) on delete cascade,
    username      text not null,
    password_hash text not null,
    created_at    text not null default (datetime('now'))
);

-- Unicité portée par un index **nommé** plutôt que par un `unique` en ligne :
-- SQLite ne sait pas supprimer une contrainte de table, mais sait supprimer un
-- index. La migration passkeys (ADR-0006) lève cette unicité — il lui faut donc
-- un nom à viser.
create unique index users_username_key on users (username);

create index users_household_id_idx on users (household_id);

-- Lien d'invitation : token à usage unique et expirant.
-- Le bootstrap du 1er compte passe par BOOTSTRAP_INVITE_CODE (env), pas par cette table.
create table invitations (
    id           blob primary key,
    household_id blob not null references households (id) on delete cascade,
    created_by   blob not null references users (id) on delete cascade,
    token        text not null,
    expires_at   text not null,
    consumed_at  text,
    consumed_by  blob references users (id) on delete set null,
    created_at   text not null default (datetime('now'))
);

create unique index invitations_token_key on invitations (token);

create index invitations_household_id_idx on invitations (household_id);
