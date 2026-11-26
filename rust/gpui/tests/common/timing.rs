//! Release gates corresponding to tauri-dev-mock-timing.ts. No wall-clock sleeps.
use super::*;
use tokio::sync::oneshot;

type LoadResult = Result<serde_json::Value, AppError>;

#[derive(Clone, Default)]
pub(super) struct Timing {
    rules: Rules,
    /// Fails every read, as the desktop backend's remote route does once the selected server drops.
    refusal: std::sync::Arc<Mutex<Option<AppError>>>,
}

#[derive(Default)]
struct Rule {
    held: bool,
    requests: u64,
    failed_at: Option<u64>,
    failure: Option<AppError>,
    loads: HashMap<String, Load>,
}

#[derive(Default)]
struct Load {
    dirty: bool,
    snapshot: Option<serde_json::Value>,
    waiters: Vec<oneshot::Sender<LoadResult>>,
}

type Rules = std::sync::Arc<Mutex<HashMap<(String, String), Rule>>>;

/// One caller's wait on a held load. Dropping it, when the read completes or
/// its caller cancels, removes the load once no caller waits on it, as the
/// browser mock's `load` does when its last waiter leaves. A cancelled read is
/// then no longer pending, and `competing_reads` does not restart it.
struct Waiter {
    receiver: Option<oneshot::Receiver<LoadResult>>,
    rules: Rules,
    rule: (String, String),
    key: String,
}

impl Drop for Waiter {
    fn drop(&mut self) {
        // A scenario assertion that failed under this lock poisoned it; a
        // waiter dropped while that panic unwinds must not panic again.
        let mut rules = self
            .rules
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Close this waiter's channel under the lock, so its sender reads as
        // closed below and no caller joins the abandoned load in between.
        drop(self.receiver.take());
        let Some(rule) = rules.get_mut(&self.rule) else {
            return;
        };
        let Some(load) = rule.loads.get_mut(&self.key) else {
            return;
        };
        // A load released since then is gone, and a later load with the same
        // key keeps every waiter whose caller is still waiting.
        load.waiters.retain(|sender| !sender.is_closed());
        if load.waiters.is_empty() {
            rule.loads.remove(&self.key);
        }
    }
}

impl Timing {
    pub(super) fn wrap<T: Send + 'static>(
        &self,
        command: &str,
        context: &str,
        read: Read<T>,
    ) -> Read<T> {
        let gate = self.read(command, context, "", ());
        Box::pin(async move {
            gate.await?;
            read.await
        })
    }
    pub(super) fn refuse(&self, error: Option<AppError>) {
        *self.refusal.lock().unwrap() = error;
    }
    pub(super) fn invalidate(&self, context: &str) {
        for ((command, rule_context), rule) in self.rules.lock().unwrap().iter_mut() {
            // Streams do not participate in the finite snapshot cache.
            if rule_context == context
                && command != "credential"
                && command != "start_resource_watch"
            {
                for load in rule.loads.values_mut() {
                    load.dirty = true;
                }
            }
        }
    }
    pub(super) fn control(&self, control: &serde_json::Value) {
        let command = control["command"].as_str().unwrap().to_owned();
        let context = control["context"].as_str().unwrap().to_owned();
        let mut rules = self.rules.lock().unwrap();
        let rule = rules.entry((command, context)).or_default();
        match control["operation"].as_str().unwrap() {
            "hold" => rule.held = true,
            "dirty" => {
                for load in rule.loads.values_mut() {
                    load.dirty = true;
                }
            }
            "contend" => {}
            "release" | "fail" | "failOnce" => {
                rule.held = false;
                if control["operation"] != "release" {
                    rule.failed_at = Some(rule.requests);
                }
                let error = if control["operation"] != "release" {
                    let kind = control["kind"].as_str().unwrap_or("transport");
                    let mut error = AppError::new(
                        format!("Scripted read: {kind}"),
                        serde_json::from_value(serde_json::json!(kind)).unwrap(),
                    );
                    if control["credentialPlugin"] == true {
                        error.credential = Some(Box::new(mock_credential(
                            control["context"].as_str().unwrap(),
                        )));
                    }
                    Some(error)
                } else {
                    None
                };
                rule.failure = if control["operation"] == "fail" {
                    error.clone()
                } else {
                    None
                };
                let selected: Vec<_> = rule
                    .loads
                    .keys()
                    .filter(|key| {
                        control["namespace"]
                            .as_str()
                            .is_none_or(|ns| key.as_str() == ns)
                    })
                    .cloned()
                    .collect();
                for key in selected {
                    let mut load = rule.loads.remove(&key).unwrap();
                    let result = match &error {
                        Some(error) => Err(error.clone()),
                        None => Ok(load.snapshot.take().expect("pending snapshot")),
                    };
                    for sender in load.waiters.drain(..) {
                        let _ = sender.send(result.clone());
                    }
                }
            }
            "expectRequests" => assert_eq!(rule.requests, control["count"].as_u64().unwrap()),
            "expectNoError" => {}
            "expectNoRetry" => assert_eq!(Some(rule.requests), rule.failed_at),
            "expectPending" => assert!(
                rule.loads
                    .keys()
                    .any(|key| control["namespace"].as_str().is_none_or(|ns| key == ns)),
                "No pending read"
            ),
            operation => panic!("Unknown timing operation: {operation}"),
        }
    }

    pub(super) fn read<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
        &self,
        command: &str,
        context: &str,
        key: &str,
        result: T,
    ) -> Read<T> {
        if let Some(error) = self.refusal.lock().unwrap().clone() {
            return Box::pin(async move { Err(error) });
        }
        let credential = self.gate("credential", context, "", ());
        let timing = self.clone();
        let command = command.to_owned();
        let context = context.to_owned();
        let key = key.to_owned();
        Box::pin(async move {
            credential.await?;
            timing.gate(&command, &context, &key, result).await
        })
    }

    /// Holds or fails `command` as scripted, without the credential gate or the
    /// remote refusal that cluster reads get.
    pub(super) fn gate<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
        &self,
        command: &str,
        context: &str,
        key: &str,
        result: T,
    ) -> Read<T> {
        let mut rules = self.rules.lock().unwrap();
        let Some(rule) = rules.get_mut(&(command.into(), context.into())) else {
            return Box::pin(async move { Ok(result) });
        };
        rule.requests += 1;
        if let Some(error) = rule.failure.clone() {
            return Box::pin(async move { Err(error) });
        }
        if !rule.held {
            return Box::pin(async move { Ok(result) });
        }
        let load = rule.loads.entry(key.into()).or_default();
        // live_store.rs restarts dirty in-flight loads on the next caller.
        if load.dirty {
            for sender in load.waiters.drain(..) {
                let _ = sender.send(Err(AppError::cancelled()));
            }
            load.dirty = false;
            load.snapshot = None;
        }
        load.snapshot
            .get_or_insert_with(|| serde_json::to_value(&result).unwrap());
        let (sender, receiver) = oneshot::channel();
        load.waiters.push(sender);
        let mut waiter = Waiter {
            receiver: Some(receiver),
            rules: self.rules.clone(),
            rule: (command.into(), context.into()),
            key: key.into(),
        };
        Box::pin(async move {
            let receiver = waiter
                .receiver
                .as_mut()
                .expect("a waiter holds its receiver");
            let snapshot = receiver.await.map_err(|_| AppError::cancelled())??;
            serde_json::from_value(snapshot)
                .map_err(|error| AppError::new(error.to_string(), AppErrorKind::Serialization))
        })
    }
}

impl Fixture {
    // The parity driver polls these reads as independent backend consumers.
    #[allow(dead_code)]
    pub fn competing_reads(&self, control: &serde_json::Value) -> Vec<Read<()>> {
        let command = control["command"].as_str().unwrap();
        let context = control["context"].as_str().unwrap();
        let keys: Vec<_> = self
            .timing
            .rules
            .lock()
            .unwrap()
            .get(&(command.into(), context.into()))
            .map_or_default(|rule| rule.loads.keys().cloned().collect());
        keys.into_iter()
            .map(|key| -> Read<()> {
                let scope = Scope {
                    context: context.into(),
                    namespace: (!key.is_empty()).then_some(key),
                };
                match command {
                    "list_resource_topology" => {
                        let read = self.topology(scope);
                        Box::pin(async move { read.await.map(|_| ()) })
                    }
                    "list_resource_scope" => {
                        let read = self.resources(scope, false);
                        Box::pin(async move { read.await.map(|_| ()) })
                    }
                    "list_resource_kinds" => {
                        let read = self.resource_kinds(context.into());
                        Box::pin(async move { read.await.map(|_| ()) })
                    }
                    _ => panic!("No competing fixture read for {command}"),
                }
            })
            .collect()
    }
    // The visual and native fixtures share this module without driving scripts.
    #[allow(dead_code)]
    pub fn timing_control(&self, control: &serde_json::Value) {
        self.timing.control(control);
    }
}

/// `mockCredential` in `src/lib/tauri-dev-mock-timing.ts`.
fn mock_credential(context: &str) -> kubecove_lib::models::CredentialFailure {
    kubecove_lib::models::CredentialFailure {
        context: context.to_owned(),
        user: Some("oidc".to_owned()),
        command: "kubectl oidc-login get-token".to_owned(),
        kubeconfig: vec!["/home/user/.kube/config".to_owned()],
        reproduce: format!("kubectl --context {context} auth whoami"),
        exit_status: Some(1),
        plugin_message: Some(
            "error: oidc: issuer URL did not match the issuer URL returned by provider".to_owned(),
        ),
    }
}
