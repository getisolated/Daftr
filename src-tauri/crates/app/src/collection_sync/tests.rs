mod paths;
mod plans;
mod sim;
mod simulation;

use super::*;

pub(super) fn hash(text: &str) -> FileHash {
    FileHash::of(text.as_bytes())
}

pub(super) fn path(text: &str) -> SyncPath {
    SyncPath::parse(text, &[]).unwrap()
}
