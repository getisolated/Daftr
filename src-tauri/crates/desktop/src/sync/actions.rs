//! Ce que font les commandes de la synchronisation, sans Tauri : vue d'ensemble, appareils,
//! appairage, offres, retraits. Les entrées sont déjà validées par
//! `presentation::sync`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use adapter_sync::{Fingerprint, LinkedDevice, OfferChoice, PairingState, PairingTicket};
use app::ports::sync::{SyncProfiles, SyncStore};
use app::ports::trust::{TrustStore, TrustedDevice};
use app::{AppResult, SyncError};
use domain::i18n::{MessageKey, message};
use domain::profile::ProfileId;
use domain::settings::AppSettings;
use domain::sync::{DeviceKey, DeviceKind, PairingCode, SpaceId};
use presentation::IpcResult;
use presentation::dto::{
    LinkedDeviceDto, PairingStateDto, PairingTicketDto, ProfileSyncDto, SharedProfileDto,
    SpaceDeviceDto, SyncStateDto, SyncedProfileDto, TrustedDeviceDto,
};
use presentation::sync::{OfferAnswer, linked_device};

use super::{DesktopSyncProfiles, SyncState, ipc_error};

impl SyncState {
    /// Vue d'ensemble de Réglages → Synchronisation.
    pub fn overview(&self, settings: &AppSettings) -> SyncStateDto {
        let profiles = self
            .profiles
            .synced_profiles()
            .unwrap_or_default()
            .into_iter()
            .map(|synced| SyncedProfileDto {
                profile_id: synced.profile.as_str().to_owned(),
                space_id: synced.space_id.to_string(),
                enabled: synced.enabled,
            })
            .collect();
        SyncStateDto {
            status: (&self.engine.status()).into(),
            device_key: self.device_key().to_string(),
            device_name: self.device_name(settings).as_str().to_owned(),
            device_name_is_default: settings.sync_device_name.is_empty(),
            port: settings.sync_port,
            listening_port: self.engine.listening_port(),
            paused: settings.sync_paused,
            profiles,
        }
    }

    /// Appareils de confiance : présence, profils partagés, dernier échange.
    pub async fn devices(&self) -> IpcResult<Vec<TrustedDeviceDto>> {
        let trust = Arc::clone(&self.trust);
        let trusted = blocking(move || trust.devices()).await?;
        let present = self.engine.present_devices().await.unwrap_or_default();
        let connected = self.engine.connected_devices();
        let mut shared = self.on_profiles(shared_profiles).await?;

        Ok(trusted
            .iter()
            .map(|device| {
                let presence = (
                    present.contains(&device.key),
                    connected.contains(&device.key),
                );
                let profiles = shared.remove(&device.key).unwrap_or_default();
                TrustedDeviceDto::new(device, fingerprint(&device.key), presence, profiles)
            })
            .collect())
    }

    /// La synchronisation d'un profil : son espace et ses membres, avec leur type.
    pub async fn profile_sync(&self, profile: ProfileId) -> IpcResult<ProfileSyncDto> {
        self.profiles.profile_name(&profile)?;
        let space = self.profiles.space_of(&profile);
        let id = profile.clone();
        let devices = match space {
            Some(_) => {
                self.on_profiles(move |profiles| profiles.with_sqlite(&id, |store| store.devices()))
                    .await?
            }
            None => Vec::new(),
        };
        let trust = Arc::clone(&self.trust);
        let trusted = blocking(move || trust.devices()).await?;
        let own = (self.device_key(), self.device_kind);
        Ok(ProfileSyncDto {
            profile_id: profile.as_str().to_owned(),
            space_id: space.map(|space| space.space_id.to_string()),
            devices: devices
                .into_iter()
                .map(|device| {
                    let kind = kind_of(&device.key, own, &trusted);
                    SpaceDeviceDto {
                        key: device.key.to_string(),
                        fingerprint: fingerprint(&device.key),
                        name: device.name.map(|name| name.to_string()).unwrap_or_default(),
                        kind: kind.into(),
                        is_self: device.key == own.0,
                        revoked: device.revoked_at.is_some(),
                    }
                })
                .collect(),
        })
    }

    /// « Lier un appareil » : les profils cochés deviennent synchronisés (espace créé au
    /// besoin), puis le mode appairage s'ouvre.
    pub async fn start_pairing(
        &self,
        profiles: Vec<ProfileId>,
        settings: &AppSettings,
    ) -> IpcResult<PairingTicketDto> {
        self.ensure_spaces(profiles.clone(), settings).await?;
        let ticket = self
            .engine
            .start_pairing(profiles)
            .await
            .map_err(ipc_error)?;
        Ok(ticket_dto(&ticket))
    }

    /// Ferme le mode appairage.
    pub async fn stop_pairing(&self) -> IpcResult<()> {
        self.engine.stop_pairing().await.map_err(ipc_error)
    }

    /// Suivi du mode appairage, pour les événements.
    pub fn pairing(&self) -> tokio::sync::watch::Receiver<PairingState> {
        self.engine.pairing()
    }

    /// « Se lier à un appareil » par son code, vers `address` ou les appareils découverts.
    pub async fn pair(
        &self,
        code: PairingCode,
        address: Option<SocketAddr>,
    ) -> IpcResult<LinkedDeviceDto> {
        let LinkedDevice { key, name } = self
            .engine
            .pair_with_code(code, address)
            .await
            .map_err(ipc_error)?;
        Ok(linked_device(&key, &name))
    }

    /// Réponse à un profil proposé ; « nouveau profil » est refusé si l'espace a déjà son
    /// profil ici (un espace par appareil).
    pub async fn answer_offer(
        &self,
        device: DeviceKey,
        space: SpaceId,
        answer: OfferAnswer,
    ) -> IpcResult<()> {
        let choice = match answer {
            OfferAnswer::NewProfile => {
                if self.profiles.profile_for(&space)?.is_some() {
                    return Err(app::AppError::from(SyncError::AlreadySynced).into());
                }
                OfferChoice::NewProfile
            }
            OfferAnswer::Merge(profile) => {
                self.profiles.profile_name(&profile)?;
                OfferChoice::Merge(profile)
            }
            OfferAnswer::Ignore => OfferChoice::Ignore,
        };
        self.engine
            .answer_offer(device, space, choice)
            .await
            .map_err(ipc_error)
    }

    /// « Synchroniser avec… » : propose le profil (synchronisé au besoin) à un appareil.
    pub async fn offer_profile(
        &self,
        profile: ProfileId,
        device: DeviceKey,
        settings: &AppSettings,
    ) -> IpcResult<()> {
        self.ensure_spaces(vec![profile.clone()], settings).await?;
        self.engine.offer(profile, device).await.map_err(ipc_error)
    }

    /// « Arrêter de synchroniser ce profil avec <appareil> ».
    pub async fn stop_sharing(&self, profile: ProfileId, device: DeviceKey) -> IpcResult<()> {
        self.profiles.profile_name(&profile)?;
        self.engine
            .stop_sharing(profile, device)
            .await
            .map_err(ipc_error)
    }

    /// « Retirer l'appareil ».
    pub async fn remove_device(&self, device: DeviceKey) -> IpcResult<()> {
        self.engine.remove_device(device).await.map_err(ipc_error)
    }

    /// « Synchroniser maintenant ».
    pub async fn sync_now(&self) -> IpcResult<()> {
        self.engine.sync_now().await.map_err(ipc_error)
    }

    /// Un profil a été supprimé : il n'est plus synchronisé.
    pub async fn forget_profile(&self, profile: &ProfileId) {
        self.profiles.forget(profile);
        self.unwatch_profile(profile);
        self.refresh().await;
    }

    /// Rend synchronisés et activés les profils donnés, puis relance le moteur.
    async fn ensure_spaces(
        &self,
        profiles: Vec<ProfileId>,
        settings: &AppSettings,
    ) -> IpcResult<()> {
        let device_name = self.device_name(settings);
        let default_name = message(settings.language, MessageKey::SyncDefaultSpaceName);
        self.on_profiles(move |all| {
            for profile in &profiles {
                let name = all.profile_name(profile)?;
                let name = name.as_deref().unwrap_or(default_name);
                all.ensure_space(profile, name, &device_name)?;
            }
            Ok(())
        })
        .await?;
        self.engine.refresh().await.map_err(ipc_error)
    }

    /// Exécute `work` sur les profils hors des threads du runtime (accès SQLite).
    pub(super) async fn on_profiles<T, F>(&self, work: F) -> IpcResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&DesktopSyncProfiles) -> AppResult<T> + Send + 'static,
    {
        let profiles = Arc::clone(&self.profiles);
        blocking(move || work(&profiles)).await
    }
}

pub(super) async fn blocking<T, F>(work: F) -> IpcResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => Ok(result?),
        Err(_) => Err(ipc_error(adapter_sync::Error::Stopped)),
    }
}

/// L'empreinte de `key`, en hexadécimal.
fn fingerprint(key: &DeviceKey) -> String {
    Fingerprint::of(key).to_string()
}

/// Le type d'un membre d'espace : le sien pour cet appareil (`own`), sinon celui de
/// l'appareil de confiance.
fn kind_of(key: &DeviceKey, own: (DeviceKey, DeviceKind), trusted: &[TrustedDevice]) -> DeviceKind {
    if *key == own.0 {
        return own.1;
    }
    trusted
        .iter()
        .find(|device| device.key == *key)
        .map_or(DeviceKind::Unknown, |device| device.kind)
}

/// Les profils partagés avec chaque appareil (membres de l'espace, révoqués compris).
fn shared_profiles(
    profiles: &DesktopSyncProfiles,
) -> AppResult<HashMap<DeviceKey, Vec<SharedProfileDto>>> {
    let mut shared: HashMap<DeviceKey, Vec<SharedProfileDto>> = HashMap::new();
    for synced in profiles.synced_profiles()? {
        let profile_id = synced.profile.as_str().to_owned();
        let members = profiles.with_sqlite(&synced.profile, |store| {
            store
                .devices()?
                .into_iter()
                .map(|device| Ok((store.peer(&device.key)?, device)))
                .collect::<AppResult<Vec<_>>>()
        })?;
        for (peer, device) in members {
            shared
                .entry(device.key)
                .or_default()
                .push(SharedProfileDto {
                    profile_id: profile_id.clone(),
                    revoked: device.revoked_at.is_some(),
                    last_sync_at: peer.and_then(|peer| peer.last_seen_at),
                });
        }
    }
    Ok(shared)
}

/// Le mode appairage pour le renderer.
pub fn pairing_state_dto(state: &PairingState) -> PairingStateDto {
    match state {
        PairingState::Inactive => PairingStateDto {
            active: false,
            ticket: None,
        },
        PairingState::Active(ticket) => PairingStateDto {
            active: true,
            ticket: Some(ticket_dto(ticket)),
        },
    }
}

fn ticket_dto(ticket: &PairingTicket) -> PairingTicketDto {
    PairingTicketDto {
        code: ticket.code.as_str().to_owned(),
        uri: ticket.uri.to_string(),
        qr_svg: ticket.qr_svg.clone(),
        expires_at: unix_millis(ticket.expires_at),
    }
}

/// L'instant (horloge monotone) en millisecondes depuis l'époque Unix.
fn unix_millis(at: Instant) -> u64 {
    let remaining = at.saturating_duration_since(Instant::now());
    let wall = SystemTime::now() + remaining;
    let since_epoch = wall.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX)
}
