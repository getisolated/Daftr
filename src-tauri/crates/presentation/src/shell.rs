//! Ouverture d'adresses externes.

use app::Refusal;

use crate::IpcResult;

/// L'adresse si elle est web (`http://` ou `https://`) : rien d'autre (fichiers, protocoles
/// d'applications) ne s'ouvre depuis le renderer.
pub fn external_url(url: &str) -> IpcResult<&str> {
    let is_web = url.starts_with("http://") || url.starts_with("https://");
    if is_web {
        Ok(url)
    } else {
        Err(Refusal::UnsupportedUrl.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_only_web_addresses() {
        assert!(external_url("https://example.com").is_ok());
        assert!(external_url("http://localhost").is_ok());
        assert!(external_url("file:///C:/Windows").is_err());
        assert!(external_url("ms-settings:").is_err());
    }
}
