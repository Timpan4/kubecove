//! The developer topology harness that Diagnostics opens in debug builds.
use super::*;

fn spike_labels(window: &gpui_kit::Window) -> Vec<String> {
    ["topology-spike-counts", "topology-spike-result"]
        .into_iter()
        .filter_map(|id| window.try_find(id))
        .filter_map(|element| element.label().map(str::to_owned))
        .collect()
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "runTopologySpike" => {
            window.click("topology-spike-run", cx);
            // The run advances one step per display frame, as the web's requestAnimationFrame
            // loop does. It ends when no step asks for another frame.
            loop {
                window.render_frame(cx);
                if window.simulate_next_frame(cx) == 0 {
                    break;
                }
            }
            window.render_frame(cx);
            assert!(
                window.try_find("topology-spike-result").is_some(),
                "the benchmark run finished without a result"
            );
        }
        "expectTopologySpikeText" => {
            let labels = spike_labels(window);
            assert!(
                labels
                    .iter()
                    .any(|label| label.contains(string(step, "value"))),
                "Missing topology spike text {:?}; rendered: {labels:?}",
                string(step, "value")
            );
        }
        _ => return false,
    }
    true
}
