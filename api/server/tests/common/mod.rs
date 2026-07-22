//! Base SQLite jetable, partagée par les tests d'intégration.
//!
//! Depuis l'ADR-0008 la base est un simple fichier : chaque test s'en donne un
//! neuf dans un dossier temporaire et y joue les migrations. Plus de
//! `docker compose` à lancer, donc plus de `#[ignore]` — et plus de foyer de
//! démo partagé entre deux exécutions, dont il fallait nettoyer les traces.

#![allow(dead_code)]

use sqlx::SqlitePool;
use tempfile::TempDir;

/// Base temporaire : le dossier est supprimé — et la base avec — quand cette
/// valeur est libérée. La garder vivante aussi longtemps que le pool.
pub struct TempDatabase {
    /// Pool ouvert sur la base migrée.
    pub pool: SqlitePool,
    _dir: TempDir,
}

/// Crée une base neuve, migrations jouées.
pub async fn temp_database() -> TempDatabase {
    let dir = tempfile::tempdir().expect("dossier temporaire");
    let path = dir.path().join("weekmeals.db");
    let pool = server::pool(&format!("sqlite://{}", path.display())).expect("pool SQLite");
    server::migrate(&pool).await.expect("migrations");
    TempDatabase { pool, _dir: dir }
}
