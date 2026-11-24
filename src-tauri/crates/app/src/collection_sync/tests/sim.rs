//! Deux copies d'une collection en mémoire, qui échangent des manifestes et appliquent les
//! plans comme le moteur réseau : même cœur pur, sans réseau.

use std::collections::{BTreeMap, HashMap};

use super::*;

/// Une copie : fichiers, bases (par chemin) avec leur texte, corbeille, versions remplacées
/// que le pair peut encore demander.
#[derive(Clone)]
pub(super) struct Replica {
    pub key: u8,
    pub files: BTreeMap<String, Vec<u8>>,
    pub bases: HashMap<String, (FileHash, Option<Vec<u8>>)>,
    pub trash: Vec<Vec<u8>>,
    pub history: HashMap<(String, FileHash), Vec<u8>>,
}

impl Replica {
    pub fn new(key: u8) -> Self {
        Self {
            key,
            files: BTreeMap::new(),
            bases: HashMap::new(),
            trash: Vec::new(),
            history: HashMap::new(),
        }
    }

    pub fn name(&self) -> String {
        format!("dev{}", self.key)
    }

    pub fn manifest(&self) -> Manifest {
        let files = self
            .files
            .iter()
            .map(|(name, bytes)| LocalFile {
                path: path(name),
                hash: FileHash::of(bytes),
                size: bytes.len() as u64,
            })
            .collect();
        let bases = self
            .bases
            .iter()
            .map(|(name, (hash, _))| (path(name), *hash))
            .collect();
        build_manifest(files, &bases)
    }

    /// La version `hash` de `name`, si on l'a (courante ou remplacée).
    fn serve(&self, name: &str, hash: FileHash) -> Option<Vec<u8>> {
        let current = self
            .files
            .get(name)
            .filter(|bytes| FileHash::of(bytes) == hash);
        current
            .or_else(|| self.history.get(&(name.to_owned(), hash)))
            .cloned()
    }

    fn put(&mut self, name: &str, bytes: Vec<u8>) {
        if let Some(old) = self.files.insert(name.to_owned(), bytes) {
            self.history
                .insert((name.to_owned(), FileHash::of(&old)), old);
        }
    }

    fn remember(&mut self, name: &str, bytes: &[u8]) {
        let text = (bytes.len() <= MAX_BASE_TEXT_BYTES).then(|| bytes.to_vec());
        self.bases
            .insert(name.to_owned(), (FileHash::of(bytes), text));
    }

    /// L'accusé du pair : il a maintenant `hash` (ou plus rien) pour ce chemin.
    fn acked(&mut self, name: &str, hash: Option<FileHash>) {
        match (hash, self.files.get(name).cloned()) {
            (Some(hash), Some(bytes)) if FileHash::of(&bytes) == hash => {
                self.remember(name, &bytes)
            }
            (Some(hash), _) => {
                self.bases.insert(name.to_owned(), (hash, None));
            }
            (None, None) => {
                self.bases.remove(name);
            }
            (None, Some(_)) => {}
        }
    }

    /// Les contenus modifiés ici depuis le dernier échange (à ne jamais perdre).
    pub fn unsynced(&self) -> Vec<Vec<u8>> {
        self.files
            .iter()
            .filter(|(name, bytes)| {
                self.bases.get(*name).map(|(hash, _)| *hash) != Some(FileHash::of(bytes))
            })
            .map(|(_, bytes)| bytes.clone())
            .collect()
    }

    /// Tout ce que cette copie garde, corbeille et versions remplacées comprises.
    pub fn everything(&self) -> Vec<Vec<u8>> {
        let mut all: Vec<Vec<u8>> = self.files.values().cloned().collect();
        all.extend(self.trash.iter().cloned());
        all.extend(self.history.values().cloned());
        all
    }
}

/// Le plan de `me` face à `peer`, fait sur des instantanés.
pub(super) fn plan_of(me: &Replica, peer: &Replica) -> Vec<PlannedFile> {
    plan(&me.manifest(), &peer.manifest().entries)
}

/// Applique `planned` sur `me`, qui lit les versions chez `peer` et lui envoie les accusés.
pub(super) fn execute(me: &mut Replica, peer: &mut Replica, planned: &[PlannedFile], acks: bool) {
    let role = Role::between(&me.key, &peer.key);
    for item in planned {
        let name = item.path.as_str();
        match item.action {
            FileAction::Pull => pull(me, peer, item, acks),
            FileAction::DeleteLocal => delete_local(me, peer, item, acks),
            FileAction::Reconcile => reconcile(me, peer, item, role, acks),
            FileAction::Adopt => adopt(me, name),
            FileAction::Forget => {
                me.bases.remove(name);
            }
            FileAction::Skip(_) => {}
        }
    }
}

/// Le fichier local n'a pas bougé depuis le plan.
fn unchanged(me: &Replica, item: &PlannedFile) -> bool {
    me.files
        .get(item.path.as_str())
        .map(|bytes| FileHash::of(bytes))
        == item.local
}

fn fetch(peer: &Replica, item: &PlannedFile) -> Option<Vec<u8>> {
    let hash = item.remote?;
    peer.serve(item.path.as_str(), hash)
}

fn pull(me: &mut Replica, peer: &mut Replica, item: &PlannedFile, acks: bool) {
    let Some(bytes) = fetch(peer, item).filter(|_| unchanged(me, item)) else {
        return;
    };
    let name = item.path.as_str();
    me.put(name, bytes.clone());
    me.remember(name, &bytes);
    if acks {
        peer.acked(name, item.remote);
    }
}

fn delete_local(me: &mut Replica, peer: &mut Replica, item: &PlannedFile, acks: bool) {
    let name = item.path.as_str();
    if !unchanged(me, item) {
        return;
    }
    if let Some(old) = me.files.remove(name) {
        me.trash.push(old);
    }
    me.bases.remove(name);
    if acks {
        peer.acked(name, None);
    }
}

fn adopt(me: &mut Replica, name: &str) {
    if let Some(bytes) = me.files.get(name).cloned() {
        me.remember(name, &bytes);
    }
}

fn reconcile(me: &mut Replica, peer: &mut Replica, item: &PlannedFile, role: Role, acks: bool) {
    let Some(remote) = fetch(peer, item).filter(|_| unchanged(me, item)) else {
        return;
    };
    let name = item.path.as_str();
    let Some(local) = me.files.get(name).cloned() else {
        return;
    };
    let base = item
        .base
        .and_then(|wanted| me.bases.get(name).filter(|(hash, _)| *hash == wanted))
        .and_then(|(_, text)| text.clone());

    match resolve(role, base.as_deref(), &local, &remote) {
        Resolution::Merged(merged) => {
            me.put(name, merged.clone());
            me.remember(name, &merged);
        }
        Resolution::KeepBoth if role == Role::Follower => {
            let copy = conflict_name(&item.path, &me.name(), |c| me.files.contains_key(c));
            me.put(copy.as_str(), local);
            me.put(name, remote.clone());
            me.remember(name, &remote);
            if acks {
                peer.acked(name, item.remote);
            }
        }
        Resolution::KeepBoth => {}
    }
}

/// Un échange complet : chacun planifie sur les instantanés, puis `first` applique en premier.
pub(super) fn round(a: &mut Replica, b: &mut Replica, a_first: bool) {
    exchange(a, b, a_first, true);
}

/// Comme [`round`], les accusés pouvant se perdre.
pub(super) fn exchange(a: &mut Replica, b: &mut Replica, a_first: bool, acks: bool) {
    let plan_a = plan_of(a, b);
    let plan_b = plan_of(b, a);

    if a_first {
        execute(a, b, &plan_a, acks);
        execute(b, a, &plan_b, acks);
    } else {
        execute(b, a, &plan_b, acks);
        execute(a, b, &plan_a, acks);
    }
}

/// Actions restantes, hors fichiers écartés.
fn pending(me: &Replica, peer: &Replica) -> usize {
    let planned = plan_of(me, peer);
    planned
        .iter()
        .filter(|item| !matches!(item.action, FileAction::Skip(_)))
        .count()
}

/// Des fichiers écartés (casse en collision) expliquent que les copies diffèrent.
fn skipped(a: &Replica, b: &Replica) -> bool {
    [plan_of(a, b), plan_of(b, a)]
        .iter()
        .flatten()
        .any(|item| matches!(item.action, FileAction::Skip(_)))
}

/// Échange jusqu'au repos ; rend le nombre d'échanges, `None` s'il ne se stabilise pas.
pub(super) fn settle(a: &mut Replica, b: &mut Replica) -> Option<usize> {
    for rounds in 1..=12 {
        round(a, b, rounds % 2 == 0);
        if pending(a, b) == 0 && pending(b, a) == 0 && (a.files == b.files || skipped(a, b)) {
            return Some(rounds);
        }
    }
    None
}
