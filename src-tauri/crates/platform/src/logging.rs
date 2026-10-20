//! Journalisation (`tracing`) vers la sortie d'erreur.

use tracing_subscriber::EnvFilter;

/// Erreur d'initialisation de la journalisation.
pub type LoggingError = Box<dyn std::error::Error + Send + Sync>;

/// Installe le collecteur global ; `RUST_LOG` remplace `default_filter` quand il est défini.
pub fn init_logging(default_filter: &str) -> Result<(), LoggingError> {
    let filter =
        EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(default_filter))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
}
