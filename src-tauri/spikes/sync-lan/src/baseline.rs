//! Serveur de référence : ce que l'application embarque déjà (runtime tokio
//! multi-thread + écoute TCP), sans aucune des nouvelles crates. Sert d'étalon
//! pour mesurer le surcoût marginal en taille, RAM et CPU.

use tokio::net::TcpListener;

/// Écoute sur `0.0.0.0:port` et ferme aussitôt chaque connexion entrante.
pub async fn serve(port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("reference: écoute sur {}", listener.local_addr()?);
    loop {
        let (stream, peer) = listener.accept().await?;
        println!("reference: connexion de {peer}, fermée");
        drop(stream);
    }
}

/// Construit le même runtime que l'application (tokio multi-thread).
pub fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}

/// Port lu en premier argument, 47821 par défaut.
pub fn port_from_args() -> u16 {
    std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(47821)
}
