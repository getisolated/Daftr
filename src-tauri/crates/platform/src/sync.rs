//! Verrous tolérants à l'empoisonnement.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Verrouille `mutex`, même empoisonné : un thread qui a paniqué en le tenant ne doit pas
/// bloquer le reste de l'application ; l'état protégé reste cohérent pour nos usages (valeurs
/// remplacées d'un bloc, jamais modifiées à moitié).
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn recovers_a_poisoned_mutex() {
        let shared = Arc::new(Mutex::new(1));
        let poisoner = Arc::clone(&shared);

        let result = std::thread::spawn(move || {
            let _guard = poisoner.lock();
            std::panic::resume_unwind(Box::new("poison"));
        })
        .join();

        assert!(result.is_err());
        assert_eq!(*lock(&shared), 1);
    }
}
