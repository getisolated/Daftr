//! Registre des profils : la liste et le dernier profil actif.

use super::{Profile, ProfileId};

/// Les profils connus et le dernier actif. Toujours cohérent : le profil par défaut existe et
/// vient en premier, les identifiants sont uniques, le dernier actif est l'un d'eux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRegistry {
    profiles: Vec<Profile>,
    last_active: ProfileId,
}

impl Default for ProfileRegistry {
    /// Le profil par défaut seul, actif.
    fn default() -> Self {
        Self::new(Vec::new(), None)
    }
}

impl ProfileRegistry {
    /// Registre rendu cohérent : le profil par défaut est ajouté s'il manque (et mis en
    /// tête), un identifiant en double garde sa première occurrence, un dernier actif
    /// inconnu redevient le profil par défaut.
    pub fn new(profiles: Vec<Profile>, last_active: Option<ProfileId>) -> Self {
        let mut unique: Vec<Profile> = Vec::with_capacity(profiles.len() + 1);
        for profile in profiles {
            if !unique.iter().any(|kept| kept.id == profile.id) {
                unique.push(profile);
            }
        }
        if let Some(index) = unique.iter().position(|profile| profile.id.is_default()) {
            let default = unique.remove(index);
            unique.insert(0, default);
        } else {
            unique.insert(0, Profile::new(ProfileId::default_profile()));
        }

        let last_active = last_active
            .filter(|id| unique.iter().any(|profile| &profile.id == id))
            .unwrap_or_else(ProfileId::default_profile);
        Self {
            profiles: unique,
            last_active,
        }
    }

    /// Les profils, le profil par défaut d'abord puis dans l'ordre de création.
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// Le dernier profil actif (rouvert au lancement).
    pub fn last_active(&self) -> &ProfileId {
        &self.last_active
    }

    /// Un profil.
    pub fn get(&self, id: &ProfileId) -> Option<&Profile> {
        self.profiles.iter().find(|profile| &profile.id == id)
    }

    /// Retient le profil actif ; `false` s'il est inconnu.
    pub fn set_last_active(&mut self, id: &ProfileId) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.last_active = id.clone();
        true
    }

    /// Ajoute le profil, ou remplace celui de même identifiant.
    pub fn upsert(&mut self, profile: Profile) {
        match self.profiles.iter_mut().find(|kept| kept.id == profile.id) {
            Some(kept) => *kept = profile,
            None => self.profiles.push(profile),
        }
    }

    /// Retire un profil et le rend ; jamais le profil par défaut. Si c'était le dernier
    /// actif, le profil par défaut le devient.
    pub fn remove(&mut self, id: &ProfileId) -> Option<Profile> {
        if id.is_default() {
            return None;
        }
        let index = self.profiles.iter().position(|profile| &profile.id == id)?;
        if &self.last_active == id {
            self.last_active = ProfileId::default_profile();
        }
        Some(self.profiles.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> ProfileId {
        ProfileId::parse(text).unwrap()
    }

    fn named(text: &str, name: &str) -> Profile {
        Profile {
            name: Some(name.to_owned()),
            ..Profile::new(id(text))
        }
    }

    #[test]
    fn starts_with_the_default_profile_only() {
        let registry = ProfileRegistry::default();

        assert_eq!(registry.profiles(), [Profile::new(id("default"))]);
        assert!(registry.last_active().is_default());
    }

    #[test]
    fn adds_the_missing_default_profile_first() {
        let registry = ProfileRegistry::new(vec![named("work", "Work")], Some(id("work")));

        let ids: Vec<&str> = registry.profiles().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["default", "work"]);
        assert_eq!(registry.last_active(), &id("work"));
    }

    #[test]
    fn moves_the_default_profile_first_and_drops_duplicates() {
        let profiles = vec![
            named("work", "Work"),
            named("default", "Mine"),
            named("work", "Copy"),
        ];
        let registry = ProfileRegistry::new(profiles, None);

        assert_eq!(
            registry.profiles(),
            [named("default", "Mine"), named("work", "Work")]
        );
    }

    #[test]
    fn falls_back_to_the_default_profile_for_an_unknown_last_active() {
        let registry = ProfileRegistry::new(vec![named("work", "Work")], Some(id("gone")));
        assert!(registry.last_active().is_default());
    }

    #[test]
    fn upserts_and_switches_the_last_active() {
        let mut registry = ProfileRegistry::default();
        registry.upsert(named("work", "Work"));
        registry.upsert(named("work", "Job"));

        assert_eq!(registry.get(&id("work")), Some(&named("work", "Job")));
        assert!(registry.set_last_active(&id("work")));
        assert!(!registry.set_last_active(&id("unknown")));
        assert_eq!(registry.last_active(), &id("work"));
    }

    #[test]
    fn never_removes_the_default_profile() {
        let mut registry = ProfileRegistry::default();

        assert_eq!(registry.remove(&id("default")), None);
        assert_eq!(registry.profiles().len(), 1);
    }

    #[test]
    fn removing_the_last_active_profile_reactivates_the_default_one() {
        let mut registry = ProfileRegistry::new(vec![named("work", "Work")], Some(id("work")));

        assert_eq!(registry.remove(&id("work")), Some(named("work", "Work")));
        assert!(registry.last_active().is_default());
        assert_eq!(registry.remove(&id("work")), None);
    }
}
