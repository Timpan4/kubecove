//! Action-to-settled wall time of the GPUI client on its real `NativeBackend`, against the
//! loopback Kubernetes fixture that `scripts/perf-gpui-speed.ts` starts. Measurement only.
//!
//! Part one calls each backend read directly and splits its wall time into network time (some
//! request in flight, from `HTTP_ACTIVITY`) and backend processing. Part two drives the
//! Inspector on the headless SDK benchmark platform with production scheduling, as `frames`
//! does, and records per action: wall time to the last draw, network time, requests, UI-thread
//! work, its draw share, and the `frame-profile` spans when that feature is on.

// `frames` uses all of the shared context; this target does not call `settle`.
#[allow(dead_code)]
#[path = "frames/context.rs"]
mod context;
#[path = "frames/platform.rs"]
mod platform;

use context::FrameContext;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    backend::{Backend, LogTarget, NativeBackend, Read, Scope},
};
use kubecove_lib::commands::helpers::http_activity::{HTTP_ACTIVITY, HttpActivitySnapshot};
use kubecove_lib::models::{PodLogStreamRequest, ResourceSummary, YamlViewMode};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read as _, Write as _},
    panic::AssertUnwindSafe,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// A network action is settled once nothing has drawn and no request has been in
/// flight for this long. Local actions use the shorter window.
const NETWORK_QUIET: Duration = Duration::from_millis(1500);
const LOCAL_QUIET: Duration = Duration::from_millis(400);
const LIMIT: Duration = Duration::from_secs(20);

fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn main() {
    let Some(report) = std::env::var_os("KUBECOVE_SPEED_REPORT") else {
        println!("speed: skipped; run `bun scripts/perf-gpui-speed.ts`");
        return;
    };
    let _ = rustls::crypto::ring::default_provider().install_default();
    let contexts: Vec<String> = std::env::var("KUBECOVE_SPEED_CONTEXTS")
        .expect("contexts")
        .split(',')
        .map(str::to_owned)
        .collect();
    let control = std::env::var("KUBECOVE_SPEED_CONTROL").expect("control address");
    // Kube clients are cached per process and run their connection workers on the runtime
    // that created them, so this backend stays alive while the UI part runs.
    let (_reads_backend, backend_reads) = backend_reads(&contexts[0]);
    let actions = ui_actions(&contexts, &control);
    std::fs::write(
        &report,
        serde_json::to_vec_pretty(&json!({
            "latencyMs": std::env::var("KUBECOVE_SPEED_LATENCY_MS").ok(),
            "rows": std::env::var("KUBECOVE_SPEED_ROWS").ok(),
            "profiling": cfg!(feature = "frame-profile"),
            "debugAssertions": cfg!(debug_assertions),
            "backendReads": backend_reads,
            "actions": actions,
        }))
        .unwrap(),
    )
    .unwrap();
    println!("speed: report -> {}", report.to_string_lossy());
}

/// Runs one read to completion on a separate thread's runtime.
fn timed<T: Send + 'static>(
    runtime: &tokio::runtime::Runtime,
    label: &str,
    read: impl FnOnce() -> Read<T>,
    items: impl Fn(&T) -> usize,
) -> (Value, Option<T>) {
    eprintln!("[speed] begin backend {label}");
    let http = HTTP_ACTIVITY.snapshot();
    let started = Instant::now();
    let result = runtime.block_on(read());
    let wall = started.elapsed();
    let after = HTTP_ACTIVITY.snapshot();
    let network = after.busy_since(http);
    eprintln!("[speed] end backend {label}");
    let (ok, count, error, value) = match result {
        Ok(value) => (true, items(&value), None, Some(value)),
        Err(error) => (false, 0, Some(error.message), None),
    };
    (
        json!({
            "label": label,
            "ok": ok,
            "error": error,
            "items": count,
            "wallMs": ms(wall),
            "networkMs": ms(network),
            "backendMs": ms(wall.saturating_sub(network)),
            "requests": after.requests_since(http),
        }),
        value,
    )
}

/// Each read twice on a fresh backend: cold, then warm against its live store.
fn backend_reads(context: &str) -> (Arc<dyn Backend>, Vec<Value>) {
    let backend: Arc<dyn Backend> = Arc::new(NativeBackend::new().expect("backend runtime"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let all = Scope {
        context: context.to_owned(),
        namespace: None,
    };
    let profile = Scope {
        context: context.to_owned(),
        namespace: Some("profile".into()),
    };
    let mut results = Vec::new();
    let mut pod: Option<ResourceSummary> = None;
    for pass in ["cold", "warm"] {
        let mut push = |value: Value| {
            let mut value = value;
            value["pass"] = json!(pass);
            results.push(value);
        };
        let b = backend.clone();
        push(timed(&runtime, "contexts", || b.contexts(None), Vec::len).0);
        let (c, b) = (context.to_owned(), backend.clone());
        push(timed(&runtime, "namespaces", || b.namespaces(c, None), Vec::len).0);
        let (c, b) = (context.to_owned(), backend.clone());
        push(timed(&runtime, "resource_kinds", || b.resource_kinds(c), Vec::len).0);
        let (s, b) = (all.clone(), backend.clone());
        let (value, scope) = timed(
            &runtime,
            "scope_builtin(all)",
            || b.scope_builtin_resources(s, false),
            |r| r.resources.len(),
        );
        push(value);
        if pod.is_none() {
            pod = scope.and_then(|scope| {
                scope
                    .resources
                    .into_iter()
                    .find(|row| row.kind == "Pod" && row.name == "profile-pod-0")
            });
        }
        let (s, b) = (all.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "scope_custom(all)",
                || b.scope_custom_resources(s).expect("custom read"),
                |r| r.resources.len(),
            )
            .0,
        );
        let (s, b) = (profile.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "scope_builtin(profile)",
                || b.scope_builtin_resources(s, false),
                |r| r.resources.len(),
            )
            .0,
        );
        let (s, b) = (all.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "kind_resources(Pod, all)",
                || b.kind_resources(s, pod_key(None), Vec::new()),
                |r| r.rows.len(),
            )
            .0,
        );
        let (s, b) = (all.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "topology(all)",
                || b.topology(s),
                |t| t.nodes.len(),
            )
            .0,
        );
        let (s, b) = (profile.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "topology(profile)",
                || b.topology(s),
                |t| t.nodes.len(),
            )
            .0,
        );
        let (s, b) = (profile.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "network_topology(profile)",
                || b.network_topology(s),
                |t| t.nodes.len(),
            )
            .0,
        );
        let (s, b) = (all.clone(), backend.clone());
        push(timed(&runtime, "metrics(all)", || b.metrics(s), |m| m.pods.len()).0);
        let (c, b) = (context.to_owned(), backend.clone());
        push(
            timed(
                &runtime,
                "cluster_resources",
                || b.cluster_resources(c, false),
                |r| r.resources.len(),
            )
            .0,
        );
        let (c, b) = (context.to_owned(), backend.clone());
        push(timed(&runtime, "gitops", || b.gitops(c), |_| 0).0);
        let (c, b) = (context.to_owned(), backend.clone());
        push(
            timed(
                &runtime,
                "helm_releases",
                || b.helm_releases(c),
                |l| l.releases.len(),
            )
            .0,
        );
        let (c, b) = (context.to_owned(), backend.clone());
        push(timed(&runtime, "rbac", || b.rbac(c), |_| 0).0);
        let (c, b) = (context.to_owned(), backend.clone());
        push(timed(&runtime, "incidents", || b.incidents(c), |_| 0).0);
        let Some(pod) = pod.clone() else {
            continue;
        };
        let (r, b) = (pod.clone(), backend.clone());
        push(timed(&runtime, "details(pod)", || b.details(r), |_| 1).0);
        let (r, b) = (pod.clone(), backend.clone());
        push(
            timed(
                &runtime,
                "resource_yaml(pod)",
                || b.resource_yaml(r, YamlViewMode::Kubectl),
                String::len,
            )
            .0,
        );
        let (r, b) = (pod.clone(), backend.clone());
        push(timed(&runtime, "events(pod)", || b.events(r, None), Vec::len).0);
        let b = backend.clone();
        let target = LogTarget::Pod(PodLogStreamRequest {
            batch_lines: true,
            cluster_context: context.to_owned(),
            kubeconfig_env_var: None,
            namespace: "profile".into(),
            pod_name: pod.name.clone(),
            container: Some("app".into()),
            tail_lines: Some(500),
            since_seconds: None,
        });
        push(
            timed(
                &runtime,
                "log_stream(pod) first message",
                move || {
                    let read = b.log_stream(target);
                    Box::pin(async move {
                        let mut stream = read.await?;
                        let first = stream.messages.recv().await;
                        Ok(usize::from(first.is_some()))
                    })
                },
                |count| *count,
            )
            .0,
        );
    }
    (backend, results)
}

fn pod_key(namespace: Option<String>) -> kubecove_lib::models::WatchResourceKey {
    kubecove_lib::models::WatchResourceKey {
        resource_kind: kubecove_lib::models::WatchResourceKind {
            kind: "Pod".into(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: Some(true),
        },
        namespace,
    }
}

struct Driver {
    cx: FrameContext,
    handle: Option<AnyWindowHandle>,
    backend: Arc<dyn Backend>,
    actions: Vec<Value>,
}

fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Default::default(),
            size: size(px(1440.), px(900.)),
        })),
        focus: true,
        show: true,
        ..Default::default()
    }
}

impl Driver {
    fn open_window(&mut self, context: &str) -> AnyWindowHandle {
        let backend = self.backend.clone();
        let context = context.to_owned();
        let (handle, _) = self
            .cx
            .update(|cx| {
                gpui_kit::open_window(window_options(), cx, |window, cx| {
                    cx.new(|cx| {
                        Inspector::new(backend.clone(), window, cx).with_saved_workspace(
                            "Speed workspace",
                            context,
                            cx,
                        )
                    })
                })
            })
            .unwrap();
        handle
    }

    /// Runs `action`, then frame turns until the action has settled.
    fn measure(
        &mut self,
        label: &str,
        quiet: Duration,
        action: impl FnOnce(&mut Self) -> Result<(), String>,
    ) {
        // Clicks need targets from a current frame; draw it before the sample starts.
        if let Some(handle) = self.handle {
            let _ = self
                .cx
                .update_window(handle, |_, window, cx| window.render_frame(cx));
            self.cx.tick();
        }
        kubecove_gpui::frame_profile::take();
        eprintln!("[speed] begin {label}");
        let start_epoch = epoch_ms();
        let http = HTTP_ACTIVITY.snapshot();
        self.cx.begin_sample();
        let started = Instant::now();
        let outcome =
            std::panic::catch_unwind(AssertUnwindSafe(|| action(self))).unwrap_or_else(|panic| {
                Err(panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_else(|| "panic".into()))
            });
        let (draws, timed_out) = self.settle(started, quiet, http);
        let (settled_ns, frames, _) = self.cx.finish_sample();
        let after = HTTP_ACTIVITY.snapshot();
        eprintln!("[speed] end {label}");
        let cpu: u64 = frames.iter().map(|frame| frame.cpu).sum();
        let draw: u64 = frames.iter().map(|frame| frame.draw).sum();
        let spans = aggregate(kubecove_gpui::frame_profile::take());
        let value = json!({
            "label": label,
            "error": outcome.err(),
            "timedOut": timed_out,
            "startEpochMs": start_epoch,
            "endEpochMs": epoch_ms(),
            "wallMs": settled_ns as f64 / 1e6,
            "networkMs": ms(after.busy_since(http)),
            "requests": after.requests_since(http),
            "uiCpuMs": cpu as f64 / 1e6,
            "uiDrawMs": draw as f64 / 1e6,
            "uiNonDrawMs": cpu.saturating_sub(draw) as f64 / 1e6,
            "draws": draws.len(),
            "firstDrawMs": draws.first(),
            "drawEndsMs": &draws,
            "spans": spans,
        });
        println!(
            "speed {label}: wall={:.1}ms network={:.1}ms requests={} ui_cpu={:.1}ms draw={:.1}ms draws={}{}",
            value["wallMs"].as_f64().unwrap(),
            value["networkMs"].as_f64().unwrap(),
            value["requests"],
            value["uiCpuMs"].as_f64().unwrap(),
            value["uiDrawMs"].as_f64().unwrap(),
            draws.len(),
            if timed_out { " TIMED OUT" } else { "" },
        );
        self.actions.push(value);
    }

    fn settle(
        &mut self,
        started: Instant,
        quiet: Duration,
        mut http: HttpActivitySnapshot,
    ) -> (Vec<f64>, bool) {
        let mut quiet_since = Instant::now();
        let mut draws = Vec::new();
        loop {
            let (_, drew) = self.cx.tick();
            let now = Instant::now();
            if drew {
                draws.push(ms(now.duration_since(started)));
                quiet_since = now;
            }
            let snapshot = HTTP_ACTIVITY.snapshot();
            if snapshot != http {
                http = snapshot;
                quiet_since = now;
            }
            if now.duration_since(quiet_since) >= quiet {
                return (draws, false);
            }
            if now.duration_since(started) >= LIMIT {
                return (draws, true);
            }
            if !drew {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn click(&mut self, id: &str) -> Result<(), String> {
        let id = id.to_owned();
        self.cx
            .update_window(self.handle.expect("open window"), |_, window, cx| {
                // A click on a hidden target panics inside the window update and loses the window.
                if window
                    .try_find(id.clone())
                    .is_some_and(|target| target.visible())
                {
                    window.click(id.clone(), cx);
                    Ok(())
                } else {
                    Err(format!("{id} is not visible"))
                }
            })
            .map_err(|error| error.to_string())?
    }

    fn input(&mut self, text: &str) -> Result<(), String> {
        let text = text.to_owned();
        self.cx
            .update_window(self.handle.expect("open window"), |_, window, cx| {
                window.input(&text, cx)
            })
            .map_err(|error| error.to_string())
    }

    fn press(&mut self, key: &str) -> Result<(), String> {
        let key = key.to_owned();
        self.cx
            .update_window(self.handle.expect("open window"), |_, window, cx| {
                window.press(&key, cx)
            })
            .map_err(|error| error.to_string())
    }

    fn step(&mut self, label: &str, quiet: Duration, id: &str) {
        let id = id.to_owned();
        self.measure(label, quiet, move |driver| driver.click(&id));
    }
}

fn aggregate(spans: Vec<kubecove_gpui::frame_profile::Span>) -> Value {
    let mut totals: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    for span in serde_json::to_value(spans).unwrap().as_array().unwrap() {
        let key = format!(
            "{}/{}",
            span["component"].as_str().unwrap(),
            span["phase"].as_str().unwrap()
        );
        let entry = totals.entry(key).or_default();
        entry.0 += 1;
        entry.1 += span["ms"].as_f64().unwrap();
    }
    let mut ordered: Vec<_> = totals.into_iter().collect();
    ordered.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
    Value::Array(
        ordered
            .into_iter()
            .map(|(key, (count, total))| json!({"span": key, "count": count, "ms": total}))
            .collect(),
    )
}

/// Sends a Pod watch burst through the fixture's control server.
fn burst(control: &str, restarts: u32, events: usize) -> Result<(), String> {
    let mut stream = std::net::TcpStream::connect(control).map_err(|e| e.to_string())?;
    write!(
        stream,
        "GET /burst?restarts={restarts}&events={events} HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    eprintln!("[speed] burst {}", response.lines().last().unwrap_or(""));
    Ok(())
}

fn ui_actions(contexts: &[String], control: &str) -> Vec<Value> {
    let backend: Arc<dyn Backend> = Arc::new(NativeBackend::new().expect("backend runtime"));
    let mut driver = Driver {
        cx: FrameContext::new(false),
        handle: None,
        backend,
        actions: Vec::new(),
    };
    let (net, local) = (NETWORK_QUIET, LOCAL_QUIET);
    let first = contexts[0].clone();
    driver.measure("workspace.open (Workspace Overview)", net, |d| {
        d.handle = Some(d.open_window(&first));
        d.cx.update_window(d.handle.unwrap(), |_, window, _| window.activate_window())
            .map_err(|e| e.to_string())
    });
    driver.step("overview -> resources", net, "open-resources");
    driver.step("Cluster Overview", net, "nav:Cluster Overview");
    driver.step("all namespaces (Workloads)", net, "nav:Workloads");
    driver.step("namespace switch -> profile", net, "namespace:profile");
    driver.step("all namespaces again (Workloads)", net, "nav:Workloads");
    driver.measure("search focus", local, |d| d.click("search"));
    for (index, ch) in "profile-pod-12".chars().enumerate() {
        driver.measure(
            &format!("search keystroke {} '{ch}'", index + 1),
            local,
            |d| d.input(&ch.to_string()),
        );
    }
    for index in 0..14 {
        driver.measure(&format!("search backspace {}", index + 1), local, |d| {
            d.press("backspace")
        });
    }
    driver.step("sort Name (1)", local, "sort:Name");
    driver.step("sort Name (2)", local, "sort:Name");
    driver.step("sort Age", local, "sort:Age");
    driver.step("sort Name (3)", local, "sort:Name");
    driver.measure("kinds picker open", local, |d| d.click("kinds-picker"));
    driver.step("kind toggle Service off", net, "pick-kind:Service");
    driver.step("kind toggle Service on", net, "pick-kind:Service");
    driver.measure("kinds picker close", local, |d| d.press("escape"));
    for (restarts, events) in [(1, 1), (2, 100), (3, 10_000)] {
        let control = control.to_owned();
        driver.measure(&format!("watch burst {events} events"), net, move |_| {
            burst(&control, restarts, events)
        });
    }
    // Sustained: one event every 50 ms for 3 s, then settle.
    let sustained = control.to_owned();
    driver.measure("watch sustained 60 events at 20/s", net, move |d| {
        for round in 0..60 {
            burst(&sustained, 10 + round, 1)?;
            let until = Instant::now() + Duration::from_millis(50);
            while Instant::now() < until {
                let (_, drew) = d.cx.tick();
                if !drew {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        Ok(())
    });
    driver.step(
        "namespace switch -> profile (warm)",
        net,
        "namespace:profile",
    );
    driver.measure("search focus (profile)", local, |d| d.click("search"));
    driver.measure("search type 'profile-pod-0'", local, |d| {
        d.input("profile-pod-0")
    });
    driver.step("detail open Pod", net, "resource:Pod:profile:profile-pod-0");
    driver.step("detail YAML tab", net, "show-yaml");
    driver.step("detail Events tab", net, "show-events");
    driver.step("detail Logs tab", net, "show-logs");
    driver.step("detail Details tab", net, "show-details");
    driver.step("GitOps page", net, "nav:GitOps");
    driver.step("Helm page", net, "nav:Helm");
    driver.step("RBAC page", net, "nav:RBAC");
    driver.step("Incidents page", net, "nav:Incidents");
    driver.step("Workspace Overview page", net, "nav:Workspace Overview");
    if let Some(second) = contexts.get(1).cloned() {
        let old = driver.handle.expect("open window");
        driver.measure(
            "context switch (new context, warm process)",
            net,
            move |d| {
                d.cx.update_window(old, |_, window, _| window.remove_window())
                    .map_err(|e| e.to_string())?;
                d.handle = Some(d.open_window(&second));
                d.cx.update_window(d.handle.unwrap(), |_, window, _| window.activate_window())
                    .map_err(|e| e.to_string())
            },
        );
        driver.step(
            "new context: all namespaces (Workloads)",
            net,
            "nav:Workloads",
        );
    }
    driver.actions
}
