//! Découverte en mémoire : un « réseau » partagé par les moteurs d'un test, sur loopback.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use platform::sync::lock;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::{Announcement, Discovery, Sighting};
use crate::error::Result;

/// Réseau simulé : les annonces de chaque membre parviennent aux recherches de tous.
#[derive(Default)]
pub struct MemoryNetwork {
    hub: Mutex<Hub>,
}

#[derive(Default)]
struct Hub {
    next_member: u64,
    announcements: BTreeMap<u64, Announcement>,
    browsers: Vec<(u64, UnboundedSender<Sighting>)>,
}

impl Hub {
    fn broadcast(&mut self, sighting: &Sighting) {
        self.browsers
            .retain(|(_, browser)| browser.send(sighting.clone()).is_ok());
    }
}

impl MemoryNetwork {
    /// Un réseau vide.
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// Un nouvel appareil sur ce réseau.
    pub fn join(self: &Arc<Self>) -> MemoryDiscovery {
        let mut hub = lock(&self.hub);
        hub.next_member += 1;
        MemoryDiscovery {
            network: Arc::clone(self),
            member: hub.next_member,
        }
    }
}

/// Découverte d'un appareil sur un [`MemoryNetwork`] (adresses en `127.0.0.1`).
pub struct MemoryDiscovery {
    network: Arc<MemoryNetwork>,
    member: u64,
}

impl MemoryDiscovery {
    /// L'annonce en cours.
    pub fn announcement(&self) -> Option<Announcement> {
        lock(&self.network.hub)
            .announcements
            .get(&self.member)
            .cloned()
    }

    /// Une recherche est en cours.
    pub fn is_browsing(&self) -> bool {
        let hub = lock(&self.network.hub);
        hub.browsers
            .iter()
            .any(|(member, browser)| *member == self.member && !browser.is_closed())
    }

    fn seen(announcement: &Announcement) -> Sighting {
        Sighting::Seen {
            fingerprint: announcement.fingerprint,
            addrs: vec![SocketAddr::from((Ipv4Addr::LOCALHOST, announcement.port))],
            protocol: Some(announcement.protocol),
            pairing: announcement
                .pairing
                .as_ref()
                .map(|name| name.as_str().to_owned()),
        }
    }
}

impl Discovery for MemoryDiscovery {
    fn announce(&self, announcement: &Announcement) -> Result<()> {
        let mut hub = lock(&self.network.hub);
        hub.announcements.insert(self.member, announcement.clone());
        hub.broadcast(&Self::seen(announcement));
        Ok(())
    }

    fn withdraw(&self) -> Result<()> {
        let mut hub = lock(&self.network.hub);
        if let Some(announcement) = hub.announcements.remove(&self.member) {
            hub.broadcast(&Sighting::Lost {
                fingerprint: announcement.fingerprint,
            });
        }
        Ok(())
    }

    fn browse(&self) -> Result<UnboundedReceiver<Sighting>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut hub = lock(&self.network.hub);
        for announcement in hub.announcements.values() {
            // Le récepteur vient d'être créé : l'envoi ne peut pas échouer
            let _ = sender.send(Self::seen(announcement));
        }
        hub.browsers.push((self.member, sender));
        Ok(receiver)
    }

    fn shutdown(&self) {
        // La doublure ne peut pas échouer
        let _ = self.withdraw();
        lock(&self.network.hub)
            .browsers
            .retain(|(member, _)| *member != self.member);
    }
}
