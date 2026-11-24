//! The page and scope a set of watches belongs to. `sync_live` compares it
//! with the current scope on every render, so the common unchanged frame
//! compares the contexts in one pass without allocating.
use crate::inspector::Page;

pub(super) struct WatchOwner {
    pub(super) page: Page,
    pub(super) launcher: Option<u64>,
    pub(super) context: String,
    /// The contexts in the order last seen.
    contexts: Vec<String>,
    /// The same contexts sorted, so the same contexts in another order keep
    /// their watches.
    sorted: Vec<String>,
    pub(super) namespace: Option<String>,
}

/// How the current contexts compare with the ones a set of watches belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Contexts {
    Same,
    Reordered,
    Changed,
}

impl WatchOwner {
    pub(super) fn new(
        page: Page,
        launcher: Option<u64>,
        context: String,
        contexts: &[String],
        namespace: Option<String>,
    ) -> Self {
        let mut sorted = contexts.to_vec();
        sorted.sort_unstable();
        Self {
            page,
            launcher,
            context,
            contexts: contexts.to_vec(),
            sorted,
            namespace,
        }
    }

    /// The order last seen compares slice to slice. Another order looks each
    /// context up in the sorted copy. Workspace contexts are unique, so equal
    /// lengths with every context found mean the same contexts.
    pub(super) fn compare(&self, contexts: &[String]) -> Contexts {
        if self.contexts.as_slice() == contexts {
            Contexts::Same
        } else if self.sorted.len() == contexts.len()
            && contexts
                .iter()
                .all(|context| self.sorted.binary_search(context).is_ok())
        {
            Contexts::Reordered
        } else {
            Contexts::Changed
        }
    }

    /// Keeps a new order of the same contexts, so later frames compare it in
    /// one pass.
    pub(super) fn reorder(&mut self, contexts: Vec<String>) {
        self.contexts = contexts;
    }
}
