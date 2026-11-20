//! Commandes des profils.

use app::Refusal;
use app::ports::{CollectionStore, NoteImporter};
use app::profiles::ProfilesService;
use app::profiles::transfer::{self, TransferRequest, TransferSource, TransferTarget};
use domain::profile::{IconFormat, ProfileId};

use crate::data::{ActiveData, DataServices};
use crate::dto::{ProfileDto, ProfilesStateDto, TransferResultDto};
use crate::{IpcError, IpcResult};

/// L'identifiant envoyé par le renderer, s'il est valide.
pub fn parse_id(text: &str) -> IpcResult<ProfileId> {
    ProfileId::parse(text).ok_or_else(|| IpcError::invalid_input("invalid profile id"))
}

/// Les profils et le profil actif.
pub fn state(profiles: &ProfilesService, data: &ActiveData) -> ProfilesStateDto {
    let registry = profiles.registry();
    ProfilesStateDto {
        profiles: registry.profiles().iter().map(ProfileDto::from).collect(),
        active_id: data.profile().as_str().to_owned(),
        locked: data.is_locked(),
    }
}

/// Un profil protégé ne se modifie que déverrouillé, c'est-à-dire actif et ouvert : son
/// mot de passe a été donné.
pub fn ensure_editable(
    profiles: &ProfilesService,
    data: &ActiveData,
    id: &ProfileId,
) -> IpcResult<()> {
    let profile = profiles.get(id)?;
    let unlocked = &data.profile() == id && !data.is_locked();

    if profile.is_protected() && !unlocked {
        return Err(Refusal::ProfileLocked.into());
    }
    Ok(())
}

/// Crée un profil.
pub fn create(profiles: &ProfilesService, name: Option<&str>) -> IpcResult<ProfileDto> {
    Ok(ProfileDto::from(&profiles.create(name)?))
}

/// Renomme un profil.
pub fn rename(
    profiles: &ProfilesService,
    data: &ActiveData,
    id: &ProfileId,
    name: Option<&str>,
) -> IpcResult<ProfileDto> {
    ensure_editable(profiles, data, id)?;
    Ok(ProfileDto::from(&profiles.rename(id, name)?))
}

/// Remplace l'icône d'un profil.
pub fn set_icon(
    profiles: &ProfilesService,
    data: &ActiveData,
    id: &ProfileId,
    bytes: &[u8],
) -> IpcResult<ProfileDto> {
    ensure_editable(profiles, data, id)?;
    Ok(ProfileDto::from(&profiles.set_icon(id, bytes)?))
}

/// Retire l'icône d'un profil.
pub fn remove_icon(
    profiles: &ProfilesService,
    data: &ActiveData,
    id: &ProfileId,
) -> IpcResult<ProfileDto> {
    ensure_editable(profiles, data, id)?;
    Ok(ProfileDto::from(&profiles.remove_icon(id)?))
}

/// L'icône d'un profil et son type MIME, `None` sans icône.
pub fn icon(
    profiles: &ProfilesService,
    id: &ProfileId,
) -> IpcResult<Option<(&'static str, Vec<u8>)>> {
    let icon = profiles.icon(id)?;
    Ok(icon.map(|(format, bytes)| (IconFormat::mime(format), bytes)))
}

/// Définit, change ou retire le mot de passe maître.
pub fn set_password(
    profiles: &ProfilesService,
    data: &ActiveData,
    id: &ProfileId,
    current: Option<&str>,
    next: Option<&str>,
) -> IpcResult<ProfileDto> {
    ensure_editable(profiles, data, id)?;
    Ok(ProfileDto::from(&profiles.set_password(id, current, next)?))
}

/// Supprime un profil (ses données partent à la corbeille).
pub fn delete(profiles: &ProfilesService, id: &ProfileId, password: Option<&str>) -> IpcResult<()> {
    Ok(profiles.delete(id, password)?)
}

/// La base d'un autre profil, qui reçoit un transfert.
pub struct TransferDestination<'a> {
    /// Écriture des notes et dossiers.
    pub importer: &'a dyn NoteImporter,
    /// Liste des collections.
    pub collections: &'a dyn CollectionStore,
}

/// Déplace notes, dossiers et collections du profil actif vers un autre profil.
pub fn transfer(
    active: &DataServices,
    destination: &TransferDestination<'_>,
    request: &TransferRequest,
) -> IpcResult<TransferResultDto> {
    let source = TransferSource {
        notes: active.note_store.as_ref(),
        folders: active.folder_store.as_ref(),
        collections: &active.collections,
    };
    let target = TransferTarget {
        importer: destination.importer,
        collections: destination.collections,
    };
    let moved = transfer::transfer(&source, &target, request)?;

    Ok(TransferResultDto {
        note_ids: moved.note_ids,
        folder_ids: moved.folder_ids,
        collections: moved.collections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IpcErrorCode;

    #[test]
    fn refuses_an_identifier_that_could_escape_the_data_folder() {
        assert!(parse_id("a1b2").is_ok());
        let refused = parse_id("../..").unwrap_err();
        assert_eq!(refused.code, IpcErrorCode::InvalidInput);
    }
}
