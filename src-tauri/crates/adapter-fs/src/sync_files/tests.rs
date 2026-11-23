use app::ports::trust::{DeviceSecret, TrustOrigin, TrustStore, TrustedDevice};
use domain::profile::ProfileId;
use domain::sync::{DeviceKey, DeviceName, SpaceId};

use super::*;

fn device(n: u8, name: &str) -> TrustedDevice {
    TrustedDevice {
        key: DeviceKey::from_bytes([n; 32]),
        name: DeviceName::parse(name).unwrap(),
        origin: TrustOrigin::Paired,
        added_at: "2026-11-23 10:00:00".to_owned(),
        revoked_at: None,
    }
}

fn files(directory: &tempfile::TempDir) -> TrustFiles {
    TrustFiles::new(directory.path().join(SYNC_FOLDER))
}

#[test]
fn without_files_there_is_no_identity_and_no_device() {
    let directory = tempfile::tempdir().unwrap();
    let trust = files(&directory);

    assert_eq!(trust.device_secret().unwrap(), None);
    assert!(trust.devices().unwrap().is_empty());
}

#[test]
fn the_device_secret_is_saved_and_reloaded() {
    let directory = tempfile::tempdir().unwrap();
    let trust = files(&directory);
    let secret = DeviceSecret::from_bytes([7; 32]);

    trust.save_device_secret(&secret).unwrap();

    assert_eq!(trust.device_secret().unwrap(), Some(secret.clone()));
    let key_file = directory.path().join(SYNC_FOLDER).join(DEVICE_KEY_FILE);
    assert!(!temporary_path(&key_file).exists());
    // Écraser la clé repasse par un fichier neuf, lui aussi lisible
    trust.save_device_secret(&secret).unwrap();
    assert_eq!(trust.device_secret().unwrap(), Some(secret));
}

#[cfg(unix)]
#[test]
fn the_device_secret_is_readable_by_its_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    files(&directory)
        .save_device_secret(&DeviceSecret::from_bytes([7; 32]))
        .unwrap();

    let path = directory.path().join(SYNC_FOLDER).join(DEVICE_KEY_FILE);
    let mode = std::fs::metadata(path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn a_corrupted_secret_is_set_aside() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join(SYNC_FOLDER);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(DEVICE_KEY_FILE), b"short").unwrap();

    assert_eq!(files(&directory).device_secret().unwrap(), None);
    let aside = folder.join(format!("{DEVICE_KEY_FILE}{CORRUPTED_SUFFIX}"));
    assert_eq!(std::fs::read(aside).unwrap(), b"short");
    assert!(!folder.join(DEVICE_KEY_FILE).exists());
}

#[test]
fn devices_are_added_then_replaced_by_key() {
    let directory = tempfile::tempdir().unwrap();
    let trust = files(&directory);
    let laptop = device(1, "Laptop");
    let mut revoked = device(2, "Desktop");

    trust.put_device(&laptop).unwrap();
    trust.put_device(&revoked).unwrap();
    revoked.revoked_at = Some("2026-11-23 11:00:00".to_owned());
    revoked.origin = TrustOrigin::Space;
    trust.put_device(&revoked).unwrap();

    assert_eq!(trust.devices().unwrap(), [laptop, revoked]);
}

/// Un arrêt brutal pendant l'écriture laisse au pire un fichier temporaire : le fichier lu
/// est toujours la version complète précédente.
#[test]
fn an_interrupted_write_leaves_the_previous_devices() {
    let directory = tempfile::tempdir().unwrap();
    let trust = files(&directory);
    let laptop = device(1, "Laptop");
    trust.put_device(&laptop).unwrap();
    let path = directory.path().join(SYNC_FOLDER).join(DEVICES_FILE);
    assert!(!temporary_path(&path).exists());

    std::fs::write(temporary_path(&path), "{ \"devices\": [ { \"ke").unwrap();

    assert_eq!(trust.devices().unwrap(), std::slice::from_ref(&laptop));
    let phone = device(3, "Phone");
    trust.put_device(&phone).unwrap();
    assert_eq!(trust.devices().unwrap(), [laptop, phone]);
}

#[test]
fn a_corrupted_device_list_is_empty_and_kept_aside() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join(SYNC_FOLDER);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(DEVICES_FILE), "{ nope").unwrap();

    assert!(files(&directory).devices().unwrap().is_empty());
    let copy = folder.join(format!("{DEVICES_FILE}{CORRUPTED_SUFFIX}"));
    assert_eq!(std::fs::read_to_string(copy).unwrap(), "{ nope");
}

#[test]
fn unreadable_entries_are_skipped_and_revocations_kept() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join(SYNC_FOLDER);
    std::fs::create_dir_all(&folder).unwrap();
    let key = DeviceKey::from_bytes([4; 32]);
    let text = format!(
        r#"{{ "version": 1, "devices": [
            {{ "key": "zz", "name": "Bad key" }},
            {{ "key": "{key}", "name": "PC\u0007", "origin": "space",
               "addedAt": "2026-11-23 10:00:00", "revokedAt": "garbage" }},
            42
        ] }}"#
    );
    std::fs::write(folder.join(DEVICES_FILE), text).unwrap();

    let devices = files(&directory).devices().unwrap();

    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].key, key);
    assert_eq!(devices[0].name.as_str(), "PC");
    assert_eq!(devices[0].origin, TrustOrigin::Space);
    assert!(!devices[0].is_trusted());
}

#[test]
fn the_space_index_is_saved_and_reloaded() {
    let directory = tempfile::tempdir().unwrap();
    let index = SpaceIndexFile::new(&directory.path().join(SYNC_FOLDER));
    let entries = vec![
        SpaceIndexEntry {
            space_id: SpaceId::from_bytes([1; 32]),
            profile: ProfileId::default_profile(),
            enabled: true,
        },
        SpaceIndexEntry {
            space_id: SpaceId::from_bytes([2; 32]),
            profile: ProfileId::parse("work").unwrap(),
            enabled: false,
        },
    ];

    assert!(index.load().is_empty());
    index.save(&entries).unwrap();

    assert_eq!(index.load(), entries);
}

#[test]
fn a_corrupted_space_index_is_empty() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join(SYNC_FOLDER);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(INDEX_FILE), [0xff, 0xfe, 0x00]).unwrap();

    assert!(SpaceIndexFile::new(&folder).load().is_empty());
}

/// Les ACL restreintes laissent la clé lisible par son propriétaire.
#[cfg(windows)]
#[test]
fn the_restricted_device_secret_stays_readable() {
    let directory = tempfile::tempdir().unwrap();
    let trust = files(&directory);
    let secret = DeviceSecret::from_bytes([9; 32]);

    trust.save_device_secret(&secret).unwrap();
    trust.save_device_secret(&secret).unwrap();

    assert_eq!(trust.device_secret().unwrap(), Some(secret));
}
