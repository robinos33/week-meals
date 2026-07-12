//! Point d'entrée du serveur Week Meals.

use std::net::SocketAddr;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("bind du listener TCP");

    println!("Week Meals API à l'écoute sur http://{addr}");
    axum::serve(listener, server::app())
        .await
        .expect("démarrage du serveur Axum");
}
