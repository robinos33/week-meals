//! Point d'entrée du serveur Week Meals.

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    // Charge `.env` s'il existe (dev local) ; absent en prod, où l'environnement
    // est injecté par la plateforme — l'erreur est donc ignorée.
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL doit être défini");
    let pool = server::pool(&database_url).expect("configuration du pool SQLite");
    server::migrate(&pool)
        .await
        .expect("migrations applicatives");
    let session_store = server::init_session_store(&pool)
        .await
        .expect("migration du store de sessions");
    let config = server::Config::from_env();

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("bind du listener TCP");

    tracing::info!("Week Meals API à l'écoute sur http://{addr}");
    axum::serve(listener, server::app(pool, session_store, &config))
        .await
        .expect("démarrage du serveur Axum");
}
