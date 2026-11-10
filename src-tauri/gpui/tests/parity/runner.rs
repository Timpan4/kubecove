//! Runs one shared scenario: a fresh fixture and window, the startup steps
//! that apply before the client starts, the fixture controls and clock
//! advances a step needs, then the step's action through `act` and `finish`.

use super::*;

/// Runs a step's checks in the window and re-raises their panic only after the update ends.
/// A panic that unwinds through `update_window` skips GPUI's end-of-update bookkeeping, so
/// effects stop flushing for the rest of the process and later scenarios fail on missing
/// elements.
fn check_in_window(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    check: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
) {
    let outcome = cx
        .update_window(handle, |_, window, cx| {
            std::panic::catch_unwind(AssertUnwindSafe(|| check(window, cx)))
        })
        .unwrap();
    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}

/// The panic text, plus the last step the driver reached.
pub(super) fn failure_message(artifacts: &OsString, payload: Box<dyn Any + Send>) -> String {
    let panic = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_else(|| "panic without a message".to_owned());
    // The step loop writes the shard-specific name, so read the same one.
    let last_step =
        fs::read_to_string(Path::new(artifacts).join(artifact_name("gpui-progress", "txt")))
            .unwrap_or_default();
    format!("{panic} (last step: {})", last_step.trim())
}

pub(super) fn run_scenario(
    cx: &mut TestAppContext,
    scenario: &Value,
    artifacts: &OsString,
    fixture: &OsString,
    window: &mut Option<AnyWindowHandle>,
) {
    let id = string(scenario, "id");
    // A fresh fixture per scenario, as the web driver reloads the page and its
    // in-memory mock state.
    let backend = Arc::new(Fixture::load(Path::new(fixture)));
    let steps = scenario["steps"].as_array().unwrap();
    // A scenario that starts without a workspace leaves the launcher as the startup view.
    let seeded = steps
        .first()
        .is_none_or(|step| step["action"] != "startWithoutWorkspace");
    // Leading phase and hold steps apply before the client starts, as the
    // web driver's first load does for shell reads.
    let startup_steps = steps
        .iter()
        .take_while(|step| {
            step["action"] == "configureShellRead"
                || (step["action"] == "mockTiming" && step["control"]["operation"] == "hold")
        })
        .count();
    for step in &steps[..startup_steps] {
        if step["action"] == "configureShellRead" {
            *backend.shell_read_phase.lock().unwrap() = string(step, "phase").to_owned();
        } else {
            backend.timing_control(&step["control"]);
        }
    }
    let initial_coverage = scenario["steps"]
        .as_array()
        .unwrap()
        .first()
        .filter(|step| {
            step["action"] == "coverageScript"
                && matches!(string(step, "script"), "loading" | "startup-failed")
        });
    if let Some(step) = initial_coverage {
        backend.configure_coverage(string(step, "script"));
    }
    let (opened, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                // The web driver's viewport.
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(1440.), px(900.)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let inspector = Inspector::new(backend.clone(), window, cx);
                    if seeded {
                        inspector.with_saved_workspace("Parity mock workspace", "mock-dev", cx)
                    } else {
                        inspector
                    }
                })
            },
        )
        .unwrap()
    });
    // Recorded so a failing scenario can close its window before the next one opens.
    *window = Some(opened);
    let mut handle = opened;
    cx.run_until_parked();
    for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
        if index < startup_steps {
            continue;
        }
        if (step["action"] == "expectArgoText"
            && matches!(
                step["text"].as_str(),
                Some("Fixture discovery unavailable" | "Fixture provider detection unavailable")
            ))
            || (step["action"] == "expectPageText"
                && step["text"] == "Scripted page refresh failure")
            || (step["action"] == "expectLiveState" && step["label"] == "Refresh failed")
            || (step["action"] == "coverageText"
                && matches!(
                    step["text"].as_str(),
                    Some(
                        "Helm details unavailable"
                            | "Helm releases unavailable"
                            | "Incidents unavailable"
                            | "Kubeconfig sources failed"
                            | "Live sessions unavailable"
                            | "RBAC inspection unavailable"
                            | "Failed to load ownership map"
                    )
                ))
            || step["action"] == "expectLauncherRetry"
            || step["action"] == "expectFriendlyError"
            || step["action"] == "expectOverviewRetry"
            || step["action"] == "expectOverviewHealth"
            || step["action"] == "expectWorkspaceReadWarning"
        {
            for millis in [1000, 2000] {
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(millis));
                cx.run_until_parked();
            }
        }
        if step["action"] == "mockTiming" {
            // A cancelled kind discovery read retries after the first retry delay.
            if step["control"]["operation"] == "expectRequests"
                || (step["control"]["operation"] == "expectNoError"
                    && step["control"]["command"] == "list_resource_kinds")
            {
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(1000));
                cx.run_until_parked();
            }
            if step["control"]["operation"] == "expectNoRetry" {
                // The web's default maximum retry delay in query-core/src/retryer.ts.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(
                        if step["control"]["command"] == "get_backend_diagnostics" {
                            1000
                        } else {
                            30_000
                        },
                    ));
                cx.run_until_parked();
            }
            if step["control"]["operation"] == "expectNoError" {
                check_in_window(cx, handle, |window, cx| {
                    window.render_frame(cx);
                    for id in [
                        "surface-error",
                        "load-error",
                        "gitops-error",
                        "argo-discovery-error",
                    ] {
                        assert!(
                            window.try_find(id).is_none(),
                            "Read error shown while retry is pending"
                        );
                    }
                });
            }
            backend.timing_control(&step["control"]);
            if step["control"]["operation"] == "contend" {
                for read in backend.competing_reads(&step["control"]) {
                    cx.background_executor
                        .spawn(async move {
                            let _ = read.await;
                        })
                        .detach();
                }
            }
            cx.run_until_parked();
            if step["control"]["operation"] == "release"
                && step["control"]["command"] == "list_resource_topology"
            {
                // read_retry.rs uses TanStack's first cancelled-read backoff.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(1000));
                cx.run_until_parked();
            }
            if step["control"]["operation"] == "release"
                && step["control"]["command"] == "start_resource_watch"
            {
                // live-view-refresh.ts coalesces changes for 250 ms.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(250));
                cx.run_until_parked();
            }
        }
        if step["action"] == "configureArgoInventory" {
            *backend.argo.inventory_mode.lock().unwrap() = string(step, "mode").into();
        }
        if step["action"] == "configureArgoMock" {
            backend.argo.configure(string(step, "script"));
            if step["script"] == "snapshot-error" {
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(15_000));
                cx.run_until_parked();
            }
        }
        if step["action"] == "configureShellRead" {
            *backend.shell_read_phase.lock().unwrap() = string(step, "phase").to_owned();
        }
        if step["action"] == "coverageScript" && (index != 0 || initial_coverage.is_none()) {
            backend.configure_coverage(string(step, "script"));
        }
        if step["action"] == "coverageCheck" && step["check"] == "emit-log" {
            backend.emit_coverage_log();
        }
        if step["action"] == "failWorkspaceContext" {
            *backend.failed_workspace_context.lock().unwrap() =
                Some(string(step, "context").to_owned());
        }
        if step["action"] == "expectMcpRequests" {
            assert_eq!(backend.mcp_requests(), strings(step, "requests"));
        }
        if step["action"] == "expectMcpCopy" {
            use kubecove_gpui::backend::Backend;
            let preferences = serde_json::to_string(&backend.load_preferences().unwrap()).unwrap();
            assert!(
                !preferences.contains("parity-mcp"),
                "MCP credentials must not enter preferences"
            );
        }
        if step["action"] == "expectDocumentationRequest" {
            assert_eq!(backend.documentation_requests(), vec![string(step, "url")]);
        }
        if step["action"] == "restartClient"
            || (step["action"] == "configureShellRead" && step["phase"] != "ready")
        {
            cx.update_window(handle, |_, window, _| window.remove_window())
                .unwrap();
            let (reopened, _) = cx.update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        // The web driver's viewport.
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: Default::default(),
                            size: size(px(1440.), px(900.)),
                        })),
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            Inspector::new(backend.clone(), window, cx).with_saved_workspace(
                                "Parity mock workspace",
                                "mock-dev",
                                cx,
                            )
                        })
                    },
                )
                .unwrap()
            });

            handle = reopened;
            *window = Some(reopened);
            cx.run_until_parked();
        }
        if step["action"] == "reopenStoredSettings" {
            backend.reopen_settings_storage();
        }
        if step["action"] == "configureSettingsMock" {
            backend.settings_script(string(step, "script"));
        }
        if step["action"] == "dropRemoteConnection" {
            backend.drop_remote_connection();
        }
        if step["action"] == "remoteSessionSignal" {
            backend.remote_signal(serde_json::from_value(step["state"].clone()).unwrap());
        }
        if step["action"] == "expectRuntimeEndpoint" {
            assert_eq!(backend.runtime_endpoint(), string(step, "value"));
        }
        if step["action"] == "expectWorkspaceExport" {
            backend.expect_workspace_export(string(step, "name"));
        }
        if step["action"] == "expectExportedShortcut" {
            backend.expect_exported_shortcut(string(step, "name"), &step["shortcut"]);
        }
        if step["action"] == "replayLiveData" {
            backend.replay_live(string(step, "script"));
        }
        if step["action"] == "expectWatchStart" {
            backend.expect_watch_start(string(step, "kind"), step["resourceVersion"].as_str());
        }
        if step["action"] == "replayPageReads" {
            backend.replay_page_reads(string(step, "phase"));
            cx.run_until_parked();
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(250));
            cx.run_until_parked();
        }
        if step["action"] == "focusStalePage" {
            gpui_kit::VisualTestContext::from_window(handle, cx).deactivate_window();
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(
                    if step["page"] == "RBAC" {
                        30_000
                    } else {
                        15_000
                    },
                ));
            cx.update_window(handle, |_, window, _| window.activate_window())
                .unwrap();
            cx.run_until_parked();
        }
        if step["action"] == "completeArgoOperation" {
            backend.complete_argo_operation();
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(15_000));
            cx.run_until_parked();
        }
        if step["action"] == "replaySessions" {
            backend.replay_sessions(string(step, "script"));
            // The live-session query interval in `portForwardLifecycle.ts`.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(3_000));
            cx.run_until_parked();
        }
        if step["action"] == "terminalOutput" {
            backend.terminal_output(string(step, "container"), string(step, "data"));
            cx.run_until_parked();
        }
        if step["action"] == "resizeViewport" {
            cx.simulate_window_resize(
                handle,
                size(
                    px(number(step, "width") as f32),
                    px(number(step, "height") as f32),
                ),
            );
            cx.run_until_parked();
        }
        if step["action"] == "terminalFit" {
            cx.simulate_window_resize(
                handle,
                size(
                    px(number(step, "width") as f32),
                    px(number(step, "height") as f32),
                ),
            );
            cx.run_until_parked();
        }
        println!("parity {id} step {index}: {}", string(step, "action"));
        fs::write(
            Path::new(&artifacts).join(artifact_name("gpui-progress", "txt")),
            format!("{id} step {index}: {}\n", string(step, "action")),
        )
        .unwrap();
        // Reads a render starts must settle before the step observes or acts.
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
        if step["action"] == "advanceClock" {
            // The tracker's tick updates the window, so advance outside the window update below.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(number(step, "ms")));
            cx.run_until_parked();
        }
        if step["action"] == "terminalRelease" {
            terminal::advance_drag_scroll(cx, number(step, "ms"));
        }
        if step["action"] == "expectLiveState" {
            // The stale-data notice appears after the show delay in
            // `inspector/loading.rs` and stays for its minimum visible time.
            // Both are 300 ms.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(300));
            cx.run_until_parked();
        }
        if step["action"] == "replayLiveData" {
            // Advance the production web refresh throttle, not wall-clock sleeps.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(250));
            cx.run_until_parked();
        }
        check_in_window(cx, handle, |window, cx| {
            window.render_frame(cx);
            act(window, cx, step);
        });
        cx.run_until_parked();
        check_in_window(cx, handle, |window, cx| {
            window.render_frame(cx);
            finish(window, cx, step);
        });
        cx.run_until_parked();
        if matches!(string(step, "action"), "editYaml" | "replaceYamlDraft") {
            // The editor lints a changed draft after its 450 ms delay.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(450));
            cx.run_until_parked();
        }
        if step["action"] == "openResources"
            && backend.failed_workspace_context.lock().unwrap().is_some()
        {
            // The failed context's read retries after read_retry.rs's delays before the page settles.
            for millis in [1000, 2000] {
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(millis));
                cx.run_until_parked();
            }
        }
    }
    let unexpected = backend.unexpected_requests();
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n"));
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}
