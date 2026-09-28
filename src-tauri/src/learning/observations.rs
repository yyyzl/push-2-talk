//! Ownership of overlapping observations, independent of the desktop adapter.
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub(super) struct Observations<K> {
    active: Mutex<HashMap<K, Arc<AtomicBool>>>,
}

impl<K: Copy + Eq + Hash> Observations<K> {
    pub(super) fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn begin(&self, target: K) -> Observation<'_, K> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut active = self.active.lock().unwrap();
        if let Some(previous) = active.insert(target, cancel.clone()) {
            previous.store(true, Ordering::SeqCst);
        }
        Observation {
            owner: self,
            target,
            cancel,
        }
    }
}

pub(super) struct Observation<'a, K: Copy + Eq + Hash> {
    owner: &'a Observations<K>,
    target: K,
    cancel: Arc<AtomicBool>,
}

impl<K: Copy + Eq + Hash> Observation<'_, K> {
    pub(super) fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
}

impl<K: Copy + Eq + Hash> Drop for Observation<'_, K> {
    fn drop(&mut self) {
        let mut active = self.owner.active.lock().unwrap();
        // A superseded task may finish its LLM work after the next one starts.
        if active
            .get(&self.target)
            .is_some_and(|current| Arc::ptr_eq(current, &self.cancel))
        {
            active.remove(&self.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_gracefully_cancels_previous_observation() {
        let observations = Observations::new();
        let old = observations.begin(1);
        let new = observations.begin(1);
        assert!(old.cancel_flag().load(Ordering::SeqCst));
        assert!(!new.cancel_flag().load(Ordering::SeqCst));
    }

    #[test]
    fn old_completion_does_not_disable_cancellation_of_its_successor() {
        let observations = Observations::new();
        let old = observations.begin(1);
        let current = observations.begin(1);
        drop(old); // Old diff/LLM completes after a new recording has been inserted.
        let _next = observations.begin(1);
        assert!(
            current.cancel_flag().load(Ordering::SeqCst),
            "the third insertion must still stop observation of the second insertion"
        );
    }

    #[test]
    fn different_targets_do_not_cancel_each_other() {
        let observations = Observations::new();
        let first = observations.begin(1);
        let second = observations.begin(2);
        drop(first);
        assert!(!second.cancel_flag().load(Ordering::SeqCst));
        let _replacement = observations.begin(2);
        assert!(second.cancel_flag().load(Ordering::SeqCst));
    }

    #[test]
    fn unpolled_future_dropped_releases_registration() {
        let observations = Arc::new(Observations::new());
        // Keep ownership outside the future so even an unpolled task has a guard.
        let observation = observations.begin(1);
        let task = async move {
            let _observation = observation;
            std::future::pending::<()>().await;
        };
        drop(task);
        assert!(observations.active.lock().unwrap().is_empty());
    }

    #[test]
    fn immediate_completion_leaves_no_stale_registration() {
        let observations = Observations::new();
        drop(observations.begin(1));
        assert!(observations.active.lock().unwrap().is_empty());
    }
}
