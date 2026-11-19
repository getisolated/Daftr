//! Réparation des cycles de dossiers créés par des déplacements concurrents (A dans B sur un
//! appareil, B dans A sur un autre).

use std::collections::{BTreeMap, BTreeSet};

use domain::uuid::Uuid;

/// Les parents sans cycle : dans chaque cycle, le dossier au plus petit uuid est rattaché à
/// la racine. Les parents doivent désigner des dossiers de la table (sinon `None`).
///
/// Chaque dossier n'ayant qu'un parent, les cycles sont disjoints : le résultat ne dépend
/// pas de l'ordre de parcours, donc tous les appareils réparent de la même façon, sans rien
/// écrire dans le document.
pub fn repair_cycles(parents: &BTreeMap<Uuid, Option<Uuid>>) -> BTreeMap<Uuid, Option<Uuid>> {
    let mut repaired = parents.clone();
    let mut settled: BTreeSet<Uuid> = BTreeSet::new();

    for &start in parents.keys() {
        let path = walk_up(parents, &settled, start);
        if let Some(cycle_root) = cycle_minimum(parents, &path) {
            repaired.insert(cycle_root, None);
        }
        settled.extend(path);
    }
    repaired
}

/// Les dossiers parcourus en remontant depuis `start`, jusqu'à la racine, un dossier déjà
/// traité ou le premier dossier revisité (exclu).
fn walk_up(
    parents: &BTreeMap<Uuid, Option<Uuid>>,
    settled: &BTreeSet<Uuid>,
    start: Uuid,
) -> Vec<Uuid> {
    let mut path = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = Some(start);

    while let Some(folder) = current {
        if settled.contains(&folder) || !seen.insert(folder) {
            break;
        }
        path.push(folder);
        current = parents.get(&folder).copied().flatten();
    }
    path
}

/// Si le chemin se termine par un cycle (le parent du dernier dossier est sur le chemin), le
/// plus petit uuid de ce cycle.
fn cycle_minimum(parents: &BTreeMap<Uuid, Option<Uuid>>, path: &[Uuid]) -> Option<Uuid> {
    let last = path.last()?;
    let parent = parents.get(last).copied().flatten()?;
    let start = path.iter().position(|&folder| folder == parent)?;

    path[start..].iter().min().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    fn tree(links: &[(u8, Option<u8>)]) -> BTreeMap<Uuid, Option<Uuid>> {
        links
            .iter()
            .map(|&(child, parent)| (id(child), parent.map(id)))
            .collect()
    }

    #[test]
    fn leaves_a_tree_untouched() {
        let parents = tree(&[(1, None), (2, Some(1)), (3, Some(2))]);
        assert_eq!(repair_cycles(&parents), parents);
    }

    #[test]
    fn breaks_a_two_folder_cycle_at_the_smallest_uuid() {
        let repaired = repair_cycles(&tree(&[(5, Some(3)), (3, Some(5))]));
        assert_eq!(repaired, tree(&[(3, None), (5, Some(3))]));
    }

    #[test]
    fn breaks_every_cycle_and_keeps_their_branches() {
        let parents = tree(&[
            (9, Some(7)),
            (7, Some(8)),
            (8, Some(9)),
            (10, Some(8)),
            (2, Some(2)),
            (4, Some(2)),
            (1, None),
        ]);
        let repaired = repair_cycles(&parents);

        assert_eq!(repaired[&id(7)], None);
        assert_eq!(repaired[&id(8)], Some(id(9)));
        assert_eq!(repaired[&id(10)], Some(id(8)));
        assert_eq!(repaired[&id(2)], None);
        assert_eq!(repaired[&id(4)], Some(id(2)));
    }

    #[test]
    fn does_not_depend_on_where_the_walk_starts() {
        // Le chemin de 6 rejoint le cycle {4, 5} sans en faire partie
        let parents = tree(&[(6, Some(5)), (5, Some(4)), (4, Some(5))]);
        let repaired = repair_cycles(&parents);
        assert_eq!(repaired, tree(&[(6, Some(5)), (5, Some(4)), (4, None)]));
    }
}
