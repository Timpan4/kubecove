//! Release gates corresponding to tauri-dev-mock-timing.ts. No wall-clock sleeps.
use super::*;
use tokio::sync::oneshot;

type LoadResult = Result<serde_json::Value, AppError>;

#[derive(Clone, Default)]
pub(super) struct Timing {
    rules: std::sync::Arc<Mutex<HashMap<(String, String), Rule>>>,
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

impl Timing {
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
                    Some(AppError::new(
                        format!("Scripted read: {kind}"),
                        serde_json::from_value(serde_json::json!(kind)).unwrap(),
                    ))
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

    fn gate<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
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
        load.waiters.retain(|sender| !sender.is_closed());
        if load.waiters.is_empty() {
            load.snapshot = None;
        }
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
        Box::pin(async move {
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
            .map(|rule| rule.loads.keys().cloned().collect())
            .unwrap_or_default();
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
