//! Test d'intégration sur un Postgres RÉEL (Testcontainers) : applique la
//! migration socle puis vérifie que le schéma multi-foyers fonctionne
//! (tables, clés étrangères, unicité). Sert de harnais aux futurs tests de
//! repos SQLx.
//!
//! Nécessite Docker sur la machine — fourni par défaut sur les runners
//! `ubuntu-latest` de la CI.

use std::path::Path;

use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

/// Applique, dans l'ordre lexicographique, tous les fichiers `.sql` du dossier
/// `api/migrations/` sur la base cible.
async fn appliquer_migrations(pool: &PgPool) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../migrations");
    let mut fichiers: Vec<_> = std::fs::read_dir(&dir)
        .expect("lecture du dossier de migrations")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    fichiers.sort();
    assert!(
        !fichiers.is_empty(),
        "aucune migration trouvée dans {dir:?}"
    );

    for fichier in fichiers {
        let sql = std::fs::read_to_string(&fichier).expect("lecture de la migration");
        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("échec de la migration {fichier:?} : {e}"));
    }
}

#[tokio::test]
async fn la_migration_socle_cree_un_schema_fonctionnel() {
    // Même image que le dev local / la prod (Neon ≈ PG16) : `gen_random_uuid()`
    // est fourni par le cœur depuis PG13, aucune extension à activer.
    let node = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("démarrage du conteneur Postgres");
    let port = node
        .get_host_port_ipv4(5432)
        .await
        .expect("port hôte du conteneur");

    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPool::connect(&url)
        .await
        .expect("connexion au Postgres du conteneur");

    appliquer_migrations(&pool).await;

    // Un foyer + un utilisateur qui le référence : valide tables + clé étrangère.
    let household_id = "11111111-1111-1111-1111-111111111111";
    sqlx::raw_sql(&format!(
        "insert into households (id, name) values ('{household_id}', 'Foyer test')"
    ))
    .execute(&pool)
    .await
    .expect("insertion du foyer");

    sqlx::raw_sql(&format!(
        "insert into users (household_id, username, password_hash) \
         values ('{household_id}', 'alice', 'hash')"
    ))
    .execute(&pool)
    .await
    .expect("insertion de l'utilisateur");

    let (count,): (i64,) =
        sqlx::query_as("select count(*) from users where household_id = $1::uuid")
            .bind(household_id)
            .fetch_one(&pool)
            .await
            .expect("comptage des utilisateurs du foyer");
    assert_eq!(count, 1);

    // La contrainte d'unicité du username doit rejeter un doublon.
    let doublon = sqlx::raw_sql(&format!(
        "insert into users (household_id, username, password_hash) \
         values ('{household_id}', 'alice', 'autre')"
    ))
    .execute(&pool)
    .await;
    assert!(
        doublon.is_err(),
        "username dupliqué : la contrainte unique doit rejeter"
    );
}
