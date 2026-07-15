-- Socle multi-foyers + auth sans email (cf. ADR-0002).
-- Toutes les données applicatives sont scopées à un household.

create table households (
    id         uuid primary key default gen_random_uuid(),
    name       text not null,
    created_at timestamptz not null default now()
);

-- Login pseudo + mot de passe (Argon2id). username unique globalement :
-- il identifie l'utilisateur au login sans sélection de foyer ni email.
create table users (
    id            uuid primary key default gen_random_uuid(),
    household_id  uuid not null references households (id) on delete cascade,
    username      text not null unique,
    password_hash text not null,
    created_at    timestamptz not null default now()
);

create index users_household_id_idx on users (household_id);

-- Lien d'invitation : token à usage unique et expirant.
-- Le bootstrap du 1er compte passe par BOOTSTRAP_INVITE_CODE (env), pas par cette table.
create table invitations (
    id           uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id) on delete cascade,
    created_by   uuid not null references users (id) on delete cascade,
    token        text not null unique,
    expires_at   timestamptz not null,
    consumed_at  timestamptz,
    consumed_by  uuid references users (id) on delete set null,
    created_at   timestamptz not null default now()
);

create index invitations_household_id_idx on invitations (household_id);
