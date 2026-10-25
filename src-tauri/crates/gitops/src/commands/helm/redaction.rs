use k8s_openapi::api::core::v1::ConfigMap;

pub(super) fn redact_configmap_release(configmap: &mut ConfigMap) {
    if let Some(data) = configmap.data.as_mut() {
        if let Some(release) = data.get_mut("release") {
            *release = "REDACTED".to_string();
        }
    }
}
