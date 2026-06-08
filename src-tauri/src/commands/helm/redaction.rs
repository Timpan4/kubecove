use k8s_openapi::api::core::v1::{ConfigMap, Secret};

pub(super) fn redact_secret_release(secret: &mut Secret) {
    if let Some(data) = secret.data.as_mut() {
        if let Some(release) = data.get_mut("release") {
            release.0 = b"REDACTED".to_vec();
        }
    }
}

pub(super) fn redact_configmap_release(configmap: &mut ConfigMap) {
    if let Some(data) = configmap.data.as_mut() {
        if let Some(release) = data.get_mut("release") {
            *release = "REDACTED".to_string();
        }
    }
}
