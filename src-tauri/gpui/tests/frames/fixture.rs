//! Scales the browser-mock fixture to the sizes the repository's benchmarks
//! use, by replicating its rows. Each copy `k` moves a row to
//! `namespace-{(k - 1) % 100}` and suffixes its name with `-{k}`, as the
//! benchmarks spread rows over `namespace-${index % 100}`. Original rows keep
//! their identity, so their details, events and logs still resolve.

use kubecove_gpui::model::WORKSPACE_KINDS;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Each size names the benchmark it comes from; the report repeats them.
pub const SIZES: &[(&str, usize, &str)] = &[
    (
        "resource table rows",
        10_000,
        "benchmarks/resource-table.bench.ts (`rows`, 10_000 over 100 namespaces)",
    ),
    (
        "ownership map nodes",
        4_000,
        "benchmarks/topology-large.bench.ts (`buildOwnershipTopology(2_000)`, 4_000 nodes)",
    ),
    (
        "events of the opened resource",
        1_000,
        "benchmarks/resource-detail.bench.ts (`events.length` 1_000, 500 warnings)",
    ),
    (
        "log lines",
        1_000,
        "src/features/resource-detail/log-helpers.ts (`MAX_RETAINED_LOG_LINES`) and benchmarks/resource-detail.bench.ts (`logLines`)",
    ),
    (
        "incident items",
        10_000,
        "benchmarks/incidents.bench.ts (`incidentItems.length` 10_000)",
    ),
    (
        "port-forward sessions",
        500,
        "benchmarks/live-sessions.bench.ts (`portForwardSessions`)",
    ),
    (
        "exec sessions",
        250,
        "benchmarks/live-sessions.bench.ts (`podExecSessions`)",
    ),
];

fn size(label: &str) -> usize {
    SIZES
        .iter()
        .find(|(name, ..)| *name == label)
        .map(|(_, count, _)| *count)
        .expect("size is listed")
}

/// Only locates the page for unmeasured setup. The driver still navigates
/// through production controls and scrolls before clicking the target.
pub fn detail_page(fixture: &Value) -> usize {
    let rows: kubecove_gpui::model::Visible = fixture["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| table_row(row))
        .map(|row| serde_json::from_value(row.clone()).unwrap())
        .collect();
    (0..rows.len().div_ceil(kubecove_gpui::model::PAGE_SIZE))
        .find(|&page| {
            kubecove_gpui::model::indexed_table_page(
                &rows,
                &kubecove_gpui::model::TableView {
                    page,
                    sort: Default::default(),
                    collapsed: &Default::default(),
                    selected: None,
                },
                |_| Default::default(),
            )
            .rows
            .iter()
            .any(|row| {
                row.kind == "Deployment"
                    && row.namespace.as_deref() == Some("payments")
                    && row.name == "payments-api"
            })
        })
        .expect("detail fixture is in the table")
}

const NAMESPACES: usize = 100;

fn replica_namespace(copy: usize) -> String {
    format!("namespace-{}", (copy - 1) % NAMESPACES)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

/// Renames a resource-shaped object, and its metrics, into copy `copy`.
fn rename(object: &mut Value, copy: usize) {
    let Some(name) = text(object, "name") else {
        return;
    };
    let namespace = replica_namespace(copy);
    let renamed = format!("{name}-{copy}");
    object["name"] = json!(renamed);
    object["namespace"] = json!(namespace);
    if let Some(metrics) = object.get_mut("metrics").filter(|value| value.is_object()) {
        metrics["name"] = json!(renamed);
        metrics["namespace"] = json!(namespace);
    }
}

fn array<'a>(fixture: &'a mut Value, path: &[&str]) -> &'a mut Vec<Value> {
    let mut value = fixture;
    for key in path {
        value = &mut value[*key];
    }
    value
        .as_array_mut()
        .unwrap_or_else(|| panic!("fixture {path:?} is an array"))
}

/// Rows the Resources table lists: namespaced workspace kinds and custom kinds.
fn table_row(row: &Value) -> bool {
    let namespaced = row["namespace"].is_string();
    let kind = row["kind"].as_str().unwrap_or_default();
    namespaced && (WORKSPACE_KINDS.contains(&kind) || row["dynamic"] == json!(true))
}

/// Repeats `templates` in renamed copies until `rows` holds `target` items.
fn fill(
    rows: &mut Vec<Value>,
    templates: &[Value],
    target: usize,
    mut copy_of: impl FnMut(&Value, usize) -> Value,
) {
    let mut index = 0;
    while rows.len() < target {
        let copy = index / templates.len() + 1;
        rows.push(copy_of(&templates[index % templates.len()], copy));
        index += 1;
    }
}

fn scale_resources(fixture: &mut Value) -> usize {
    let resources = array(fixture, &["resources"]);
    let templates: Vec<Value> = resources
        .iter()
        .filter(|row| table_row(row))
        .cloned()
        .collect();
    let mut table: Vec<Value> = templates.clone();
    fill(
        &mut table,
        &templates,
        size("resource table rows"),
        |row, copy| {
            let mut row = row.clone();
            rename(&mut row, copy);
            row
        },
    );
    resources.extend(table.drain(templates.len()..));
    let rows = resources.iter().filter(|row| table_row(row)).count();
    let namespaces = array(fixture, &["namespaces"]);
    let template = namespaces[0].clone();
    namespaces.extend((0..NAMESPACES).map(|index| {
        let mut namespace = template.clone();
        namespace["name"] = json!(format!("namespace-{index}"));
        namespace
    }));
    rows
}

/// The node id with its trailing `:{namespace}:{name}` moved into the copy.
fn replica_id(id: &str, node: &Value, copy: usize) -> String {
    let (Some(namespace), Some(name)) = (text(node, "namespace"), text(node, "name")) else {
        return format!("{id}#{copy}");
    };
    let suffix = format!(":{namespace}:{name}");
    match id.strip_suffix(&suffix) {
        Some(prefix) => format!("{prefix}:{}:{name}-{copy}", replica_namespace(copy)),
        None => format!("{id}#{copy}"),
    }
}

fn scale_topology(fixture: &mut Value) -> usize {
    let target = size("ownership map nodes");
    let topology = &mut fixture["topology"];
    let nodes = topology["nodes"].as_array().expect("nodes").clone();
    let edges = topology["edges"].as_array().expect("edges").clone();
    let templates: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["namespace"].is_string())
        .collect();
    let namespaced: std::collections::HashSet<String> = templates
        .iter()
        .filter_map(|node| text(node, "id"))
        .collect();
    let mut scaled_nodes = nodes.clone();
    let mut scaled_edges = edges.clone();
    let mut copy = 0;
    while scaled_nodes.len() < target {
        copy += 1;
        let mut ids: HashMap<String, String> = HashMap::new();
        for node in &templates {
            if scaled_nodes.len() == target {
                break;
            }
            let id = text(node, "id").expect("node id");
            let new_id = replica_id(&id, node, copy);
            let mut node = (*node).clone();
            node["id"] = json!(new_id);
            rename(&mut node, copy);
            rename(&mut node["summary"], copy);
            ids.insert(id, new_id);
            scaled_nodes.push(node);
        }
        // Edges between copied nodes move with them; edges to shared
        // cluster-scoped nodes keep that end.
        for edge in &edges {
            let source = text(edge, "source").expect("edge source");
            let target_id = text(edge, "target").expect("edge target");
            let (new_source, new_target) = (ids.get(&source), ids.get(&target_id));
            if new_source.is_none() && new_target.is_none() {
                continue;
            }
            // The last copy can stop at the target before a namespaced end is
            // copied. Only a shared cluster-scoped end may keep the original.
            if (new_source.is_none() && namespaced.contains(&source))
                || (new_target.is_none() && namespaced.contains(&target_id))
            {
                continue;
            }
            let source = new_source.cloned().unwrap_or(source);
            let target_id = new_target.cloned().unwrap_or(target_id);
            let mut edge = edge.clone();
            edge["id"] = json!(format!("{source}->{target_id}"));
            edge["source"] = json!(source);
            edge["target"] = json!(target_id);
            scaled_edges.push(edge);
        }
    }
    let known: std::collections::HashSet<String> = scaled_nodes
        .iter()
        .filter_map(|node| text(node, "id"))
        .collect();
    scaled_edges.retain(|edge| {
        text(edge, "source").is_some_and(|id| known.contains(&id))
            && text(edge, "target").is_some_and(|id| known.contains(&id))
    });
    topology["nodes"] = Value::Array(scaled_nodes);
    topology["edges"] = Value::Array(scaled_edges);
    target
}

/// Pods of one ReplicaSet in the fan-out workload, from the memory profile's
/// large fixture, which perf-map.md measured at 60 to 120 ms per map redraw.
pub const FAN_OUT_PODS: (usize, &str) = (
    10_000,
    "e2e/harness/memory-fixture.ts (`large`: 10_000 Pods owned by one ReplicaSet)",
);

/// The scaled fixture plus [`FAN_OUT_PODS`] copies of the export's first
/// ReplicaSet-owned Pod, in its namespace, each with a row and an owner edge.
/// Every edge's culling box holds the ReplicaSet card, so the map draws them
/// all wherever that card is on screen, as the large memory fixture does.
pub fn fan_out(scaled: &Value) -> Value {
    let mut fixture = scaled.clone();
    let topology = &fixture["topology"];
    let node = |id: &str| {
        topology["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|node| text(node, "id").as_deref() == Some(id))
            .expect("edge node")
    };
    let (owner, pod) = topology["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .map(|edge| {
            (
                node(edge["source"].as_str().unwrap()),
                node(edge["target"].as_str().unwrap()),
            )
        })
        .find(|(source, target)| source["kind"] == "ReplicaSet" && target["kind"] == "Pod")
        .expect("a ReplicaSet owns a Pod");
    let (owner, pod) = (text(owner, "id").unwrap(), pod.clone());
    let (namespace, name) = (
        text(&pod, "namespace").unwrap(),
        text(&pod, "name").unwrap(),
    );
    let pod_id = text(&pod, "id").unwrap();
    let prefix = pod_id
        .strip_suffix(&format!(":{namespace}:{name}"))
        .expect("pod id ends with its namespace and name");
    let row = fixture["resources"]
        .as_array()
        .expect("resource rows")
        .iter()
        .find(|row| row["kind"] == "Pod" && row["namespace"] == namespace && row["name"] == name)
        .expect("the pod has a row")
        .clone();
    let renamed = |object: &Value, copy: &str| {
        let mut object = object.clone();
        object["name"] = json!(copy);
        if let Some(metrics) = object.get_mut("metrics").filter(|value| value.is_object()) {
            metrics["name"] = json!(copy);
        }
        object
    };
    let (mut nodes, mut edges, mut rows) = (Vec::new(), Vec::new(), Vec::new());
    for index in 0..FAN_OUT_PODS.0 {
        let copy = format!("{name}-fan-{index}");
        let id = format!("{prefix}:{namespace}:{copy}");
        let mut node = renamed(&pod, &copy);
        node["id"] = json!(id);
        node["summary"] = renamed(&pod["summary"], &copy);
        nodes.push(node);
        edges.push(json!({
            "id": format!("{owner}->{id}"),
            "source": owner,
            "target": id,
            "relation": "owns",
        }));
        rows.push(renamed(&row, &copy));
    }
    array(&mut fixture, &["topology", "nodes"]).extend(nodes);
    array(&mut fixture, &["topology", "edges"]).extend(edges);
    array(&mut fixture, &["contextResources"]).extend(rows.iter().cloned());
    array(&mut fixture, &["resources"]).extend(rows);
    fixture
}

/// The workspace context at `rank` in [`contexts`]: the saved parity
/// workspace's primary context, then added contexts.
pub fn context_name(rank: usize) -> String {
    if rank == 0 {
        "mock-dev".to_owned()
    } else {
        format!("context-{rank}")
    }
}

/// The watch-burst workload in each of `count` contexts ([`context_name`]):
/// one copy of its 10,000 rows per context, with that context as `cluster`.
/// Live kind reads copy `resources` into the watched context already.
pub fn contexts(watch: &Value, count: usize) -> Value {
    let mut fixture = watch.clone();
    let template = fixture["contexts"][0].clone();
    let rows = fixture["resources"].as_array().expect("watch rows").clone();
    let (mut contexts, mut context_rows) = (Vec::new(), Vec::new());
    for rank in 0..count {
        let context = context_name(rank);
        let mut entry = template.clone();
        entry["name"] = json!(context);
        entry["isCurrent"] = json!(rank == 0);
        contexts.push(entry);
        context_rows.extend(rows.iter().map(|row| {
            let mut row = row.clone();
            row["cluster"] = json!(context);
            row
        }));
    }
    fixture["contexts"] = Value::Array(contexts);
    fixture["contextResources"] = Value::Array(context_rows);
    fixture
}

/// The opened Deployment's events, alternating its Warning and Normal event.
fn scale_events(fixture: &mut Value) -> usize {
    let target = size("events of the opened resource");
    let events = array(fixture, &["events"]);
    let entry = events
        .iter_mut()
        .find(|entry| {
            entry["kind"] == "Deployment"
                && entry["namespace"] == "payments"
                && entry["name"] == "payments-api"
        })
        .expect("payments-api events");
    let list = entry["events"].as_array_mut().expect("event list");
    let templates = list.clone();
    fill(list, &templates, target, |event, copy| {
        let mut event = event.clone();
        let message = text(&event, "message").unwrap_or_default();
        event["message"] = json!(format!("{message} (copy {copy})"));
        event
    });
    list.len()
}

fn line_count(message: &Value) -> usize {
    match message["type"].as_str() {
        Some("logLines") => message["lines"].as_array().map_or(0, Vec::len),
        Some("logLine") => 1,
        _ => 0,
    }
}

/// Repeats each stream's line batches, numbering the copied lines.
fn scale_logs(fixture: &mut Value) -> usize {
    let target = size("log lines");
    let mut total = 0;
    for stream in ["pod", "aggregate"] {
        let messages = array(fixture, &["detailTabs", "logs", stream]);
        let batches: Vec<Value> = messages
            .iter()
            .filter(|message| message["type"] == "logLines")
            .cloned()
            .collect();
        let mut lines: usize = messages.iter().map(line_count).sum();
        let mut copy = 0;
        while lines < target {
            copy += 1;
            for batch in &batches {
                if lines == target {
                    break;
                }
                let mut batch = batch.clone();
                let room = target - lines;
                let copied: Vec<Value> = batch["lines"]
                    .as_array()
                    .expect("batch lines")
                    .iter()
                    .take(room)
                    .map(|line| json!(format!("{} #{copy}", line.as_str().unwrap_or_default())))
                    .collect();
                lines += copied.len();
                batch["lines"] = Value::Array(copied);
                messages.push(batch);
            }
        }
        total = total.max(lines);
    }
    total
}

fn scale_incidents(fixture: &mut Value) -> usize {
    let items = array(fixture, &["incidents", "items"]);
    let templates = items.clone();
    fill(items, &templates, size("incident items"), |item, copy| {
        let mut item = item.clone();
        rename(&mut item["resource"], copy);
        if let Some(event) = item
            .get_mut("latestWarningEvent")
            .filter(|value| value.is_object())
        {
            event["namespace"] = json!(replica_namespace(copy));
        }
        item
    });
    items.len()
}

fn scale_sessions(fixture: &mut Value) -> (usize, usize) {
    let forwards = array(fixture, &["liveSessions", "portForwards"]);
    let templates = forwards.clone();
    fill(
        forwards,
        &templates,
        size("port-forward sessions"),
        |session, copy| {
            let mut session = session.clone();
            let port = 20_000 + copy;
            session["id"] = json!(format!(
                "{}-{copy}",
                text(&session, "id").unwrap_or_default()
            ));
            session["namespace"] = json!(replica_namespace(copy));
            session["localPort"] = json!(port);
            session["localUrl"] = json!(format!("http://127.0.0.1:{port}"));
            session
        },
    );
    let forwards = forwards.len();
    let execs = array(fixture, &["liveSessions", "execSessions"]);
    let templates = execs.clone();
    fill(execs, &templates, size("exec sessions"), |session, copy| {
        let mut session = session.clone();
        session["id"] = json!(format!(
            "{}-{copy}",
            text(&session, "id").unwrap_or_default()
        ));
        session["namespace"] = json!(replica_namespace(copy));
        session
    });
    (forwards, execs.len())
}

/// Scales the exported fixture in place and returns the counts it reached.
pub fn scale(fixture: &mut Value) -> Map<String, Value> {
    let rows = scale_resources(fixture);
    // Scope reads gained a context-specific source after the original harness.
    // Keep the measured context identical to the scaled resource snapshot.
    let resources = fixture["resources"]
        .as_array()
        .expect("resource rows")
        .clone();
    let contexts = array(fixture, &["contextResources"]);
    contexts.retain(|row| row["cluster"].as_str() != Some("mock-dev"));
    contexts.extend(resources);
    assert_eq!(
        contexts
            .iter()
            .filter(|row| row["cluster"].as_str() == Some("mock-dev") && table_row(row))
            .count(),
        rows
    );
    let nodes = scale_topology(fixture);
    let events = scale_events(fixture);
    let lines = scale_logs(fixture);
    let incidents = scale_incidents(fixture);
    let (forwards, execs) = scale_sessions(fixture);
    let counts = [
        ("resource table rows", rows),
        ("ownership map nodes", nodes),
        ("events of the opened resource", events),
        ("log lines", lines),
        ("incident items", incidents),
        ("port-forward sessions", forwards),
        ("exec sessions", execs),
    ];
    counts
        .into_iter()
        .map(|(label, count)| {
            assert_eq!(count, size(label), "{label} must reach its benchmark size");
            let source = SIZES
                .iter()
                .find(|(name, ..)| *name == label)
                .map(|entry| entry.2);
            (label.to_owned(), json!({"count": count, "source": source}))
        })
        .collect()
}
