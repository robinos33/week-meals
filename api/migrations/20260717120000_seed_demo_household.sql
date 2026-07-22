-- Foyer de démonstration pour le mode public (cf. AUTH_DISABLED).
-- Quand l'auth est désactivée (dev / preview), l'extractor AuthUser résout ce
-- foyer à UUID fixe et y scope toutes les données. `on conflict do nothing`
-- garde la migration idempotente ; en prod (auth active) la ligne dort, inerte.
--
-- Littéral `x'…'` : les UUID sont stockés en blob (cf. ADR-0008), donc
-- 00000000-0000-0000-0000-000000000001 sans les tirets.
insert into households (id, name)
values (x'00000000000000000000000000000001', 'Démo')
on conflict (id) do nothing;
