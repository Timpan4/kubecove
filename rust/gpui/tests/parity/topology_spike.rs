//! The developer topology harness that Diagnostics opens in debug builds, and in release
//! builds with the `topology-spike` feature.
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
        #[cfg(not(any(debug_assertions, feature = "topology-spike")))]
        "runTopologySpike" => {
            let _ = cx;
            panic!("this build omits the topology harness; enable the topology-spike feature")
        }
        #[cfg(any(debug_assertions, feature = "topology-spike"))]
        "runTopologySpike" => {
            window.click("topology-spike-run", cx);
            // The run advances one step per display frame, as the web's requestAnimationFrame
            // loop does. Its own schedule bounds the frames to wait for.
            let budget = super::map_gestures::inspector(window, cx)
                .read(cx)
                .topology_spike_frames_left()
                .expect("the benchmark run started");
            let mut frames = 0;
            loop {
                window.render_frame(cx);
                if window.try_find("topology-spike-result").is_some() {
                    break;
                }
                assert!(
                    frames < budget,
                    "the benchmark run showed no result within its {budget} frames"
                );
                window.simulate_next_frame(cx);
                frames += 1;
            }
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
