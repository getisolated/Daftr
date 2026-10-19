//! Protocole `collection-file` : les images des fichiers markdown d'une collection, pour
//! l'aperçu (`http://collection-file.localhost/<collection>%0A<chemin>` sous Windows).
//!
//! Seules les images d'une collection enregistrée sont servies, désignées comme tout fichier
//! de collection par un chemin relatif vérifié.

use app::{AppError, Refusal};
use domain::collection::percent_decode;
use tauri::http::{Response, StatusCode, header};
use tauri::{AppHandle, UriSchemeContext, UriSchemeResponder};

use crate::state::desktop;

/// Nom du protocole (`convertFileSrc(…, 'collection-file')` côté renderer).
pub const SCHEME: &str = "collection-file";

/// Répond en arrière-plan : la lecture du disque ne bloque pas la fenêtre.
pub fn handle(
    context: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    let path = request.uri().path().to_owned();

    let spawned = std::thread::Builder::new()
        .name("collection-file".into())
        .spawn(move || {
            responder.respond(respond(&app, &path));
        });
    if let Err(error) = spawned {
        tracing::error!(%error, "image de collection non servie");
    }
}

fn respond(app: &AppHandle, path: &str) -> Response<Vec<u8>> {
    let Some((root, relative)) = parse(path) else {
        return status(StatusCode::BAD_REQUEST);
    };
    match desktop(app)
        .services
        .collections
        .read_image(&root, &relative)
    {
        Ok((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .body(bytes)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(AppError::Refused(
            Refusal::UnknownCollection | Refusal::OutsideCollection | Refusal::NotAnImage,
        )) => status(StatusCode::FORBIDDEN),
        Err(_) => status(StatusCode::NOT_FOUND),
    }
}

/// `/<collection>%0A<chemin relatif>` → (collection, chemin).
fn parse(path: &str) -> Option<(String, String)> {
    let decoded = percent_decode(path.strip_prefix('/')?)?;
    let (root, relative) = decoded.split_once('\n')?;

    Some((root.to_owned(), relative.to_owned()))
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_collection_and_the_path() {
        let parsed = parse("/C%3A%5Crepo%0Adoc%2FImages%2Fplan%20produit.gif");
        assert_eq!(
            parsed,
            Some((
                "C:\\repo".to_owned(),
                "doc/Images/plan produit.gif".to_owned()
            ))
        );
    }

    #[test]
    fn refuses_a_malformed_request() {
        assert_eq!(parse("/no-separator"), None);
        assert_eq!(parse("/bad%zz"), None);
    }
}
