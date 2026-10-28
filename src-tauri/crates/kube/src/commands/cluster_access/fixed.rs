//! Test adapter for the source-policy seam: every selector resolves to one
//! mock client, and each client request records its intent.

use super::{ClientIntent, ClusterAccess, Source, SourcePolicy};
use crate::commands::helpers::session_scope::SessionScope;
use kube::Client;
use std::sync::{Arc, Mutex};

pub const FIXED_SOURCE_KEY: &str = "kubeconfigSource=fixed";

pub struct FixedClusterAccess {
    client: Client,
    intents: Mutex<Vec<ClientIntent>>,
}

impl FixedClusterAccess {
    pub fn new(client: Client) -> Arc<Self> {
        Arc::new(Self {
            client,
            intents: Mutex::default(),
        })
    }

    /// A session whose requests resolve every selector to this adapter.
    pub fn scope(self: &Arc<Self>) -> Arc<SessionScope> {
        SessionScope::with_source_policy(None, SourcePolicy::Fixed(Arc::clone(self)))
    }

    pub fn access(self: &Arc<Self>) -> ClusterAccess {
        ClusterAccess {
            source_key: FIXED_SOURCE_KEY.to_string(),
            source_label: Some("fixed".to_string()),
            source: Source::Fixed(Arc::clone(self)),
        }
    }

    /// The intents requested so far, in order.
    pub fn intents(&self) -> Vec<ClientIntent> {
        self.intents.lock().expect("intent log lock").clone()
    }

    pub(super) fn client(&self, intent: ClientIntent) -> Client {
        self.intents.lock().expect("intent log lock").push(intent);
        self.client.clone()
    }
}
