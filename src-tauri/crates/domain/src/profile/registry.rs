//! Registre des profils : la liste et le dernier profil actif.

use super::{Profile, ProfileId};

/// Les profils connus et le dernier actif. Toujours cohérent : il y a au moins un profil, le
/// profil par défaut (s'il existe) vient en premier, les identifiants sont uniques, le dernier
/// actif est l'un d'eux.
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
    /// Registre rendu cohérent : le profil par défaut, s'il existe, est mis en tête ; sans aucun
    /// profil, un profil par défaut vierge est ajouté (le profil par défaut peut être supprimé
    /// tant qu'un autre profil reste). Un identifiant en double garde sa première occurrence,
    /// un dernier actif inconnu redevient le premier profil.
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
        } else if unique.is_empty() {
            unique.push(Profile::new(ProfileId::default_profile()));
        }

        let last_active = last_active
            .filter(|id| unique.iter().any(|profile| &profile.id == id))
            .unwrap_or_else(|| unique[0].id.clone());
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

    /// Retire un profil et le rend, le profil par défaut compris. Si c'était le dernier actif,
    /// le premier profil restant le devient. Retirer le seul profil en laisse un par défaut,
    /// vierge : il y en a toujours au moins un.
    pub fn remove(&mut self, id: &ProfileId) -> Option<Profile> {
        let index = self.profiles.iter().position(|profile| &profile.id == id)?;
        let removed = self.profiles.remove(index);
        if self.profiles.is_empty() {
            self.profiles
                .push(Profile::new(ProfileId::default_profile()));
        }
        if &self.last_active == id {
            self.last_active = self.profiles[0].id.clone();
        }
        Some(removed)
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
    fn adds_a_blank_default_profile_only_when_there_is_no_profile() {
        let empty = ProfileRegistry::new(Vec::new(), None);
        let without_default = ProfileRegistry::new(vec![named("work", "Work")], Some(id("work")));

        assert_eq!(empty.profiles(), [Profile::new(id("default"))]);
        assert_eq!(without_default.profiles(), [named("work", "Work")]);
        assert_eq!(without_default.last_active(), &id("work"));
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
    fn falls_back_to_the_first_profile_for_an_unknown_last_active() {
        let registry = ProfileRegistry::new(vec![named("work", "Work")], Some(id("gone")));
        assert_eq!(registry.last_active(), &id("work"));

        let with_default = ProfileRegistry::new(
            vec![named("work", "Work"), named("default", "Mine")],
            Some(id("gone")),
        );
        assert!(with_default.last_active().is_default());
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
    fn removing_the_only_profile_leaves_a_blank_default_one() {
        let mut registry = ProfileRegistry::new(vec![named("default", "Mine")], None);

        assert_eq!(
            registry.remove(&id("default")),
            Some(named("default", "Mine"))
        );
        assert_eq!(registry.profiles(), [Profile::new(id("default"))]);
        assert!(registry.last_active().is_default());
    }

    #[test]
    fn removes_the_default_profile_when_another_one_remains() {
        let mut registry = ProfileRegistry::default();
        registry.upsert(named("work", "Work"));

        assert!(registry.remove(&id("default")).is_some());
        assert_eq!(registry.profiles(), [named("work", "Work")]);
        assert_eq!(registry.last_active(), &id("work"));
    }

    #[test]
    fn keeps_a_registry_without_the_default_profile_as_it_is() {
        let mut without_default = ProfileRegistry::default();
        without_default.upsert(named("work", "Work"));
        without_default.remove(&id("default"));
        let reloaded = ProfileRegistry::new(
            without_default.profiles().to_vec(),
            Some(without_default.last_active().clone()),
        );

        assert_eq!(reloaded.profiles(), [named("work", "Work")]);
    }

    #[test]
    fn removing_the_last_active_profile_reactivates_the_first_one() {
        let mut registry = ProfileRegistry::default();
        registry.upsert(named("work", "Work"));
        registry.set_last_active(&id("work"));

        assert_eq!(registry.remove(&id("work")), Some(named("work", "Work")));
        assert!(registry.last_active().is_default());
        assert_eq!(registry.remove(&id("work")), None);
    }

    #[test]
    fn removing_the_last_non_default_profile_without_a_default_one_leaves_a_blank_default() {
        let mut registry = ProfileRegistry::default();
        registry.upsert(named("work", "Work"));
        registry.set_last_active(&id("work"));
        registry.remove(&id("default"));

        registry.remove(&id("work"));

        assert_eq!(registry.profiles(), [Profile::new(id("default"))]);
    }
}
