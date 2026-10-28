use crate::models::{AppError, AppErrorKind};
use kube::{
    api::{Api, DynamicObject, PatchParams},
    Error as KubeError,
};

use super::validation::ValidatedApply;

const FIELD_MANAGER: &str = "kubecove";

pub(super) fn apply_patch_params(force_conflicts: bool) -> PatchParams {
    let params = PatchParams::apply(FIELD_MANAGER);
    if force_conflicts {
        params.force()
    } else {
        params
    }
}

pub(super) fn apply_api(
    client: kube::Client,
    validated: &ValidatedApply,
) -> Result<Api<DynamicObject>, AppError> {
    if validated.namespaced {
        let namespace = validated.request.namespace.as_deref().ok_or_else(|| {
            AppError::new(
                "namespace is required for namespaced apply",
                AppErrorKind::Validation,
            )
        })?;
        Ok(Api::namespaced_with(
            client,
            namespace,
            &validated.api_resource,
        ))
    } else {
        Ok(Api::all_with(client, &validated.api_resource))
    }
}

pub(super) fn apply_error(error: KubeError) -> AppError {
    match &error {
        KubeError::Api(api_error) if status_has_cause(api_error, "FieldManagerConflict") => {
            AppError::new(
                api_error.message.clone(),
                AppErrorKind::FieldManagerConflict,
            )
        }
        KubeError::Api(api_error) if is_immutable_field_apply_error(api_error) => {
            AppError::new(api_error.message.clone(), AppErrorKind::ImmutableField)
        }
        _ => AppError::from(error),
    }
}

fn status_has_cause(status: &kube::core::Status, reason: &str) -> bool {
    status
        .details
        .as_ref()
        .is_some_and(|details| details.causes.iter().any(|cause| cause.reason == reason))
}

fn is_immutable_field_apply_error(status: &kube::core::Status) -> bool {
    let message = status.message.to_ascii_lowercase();
    if message.contains("pod updates may not change fields")
        || message.contains("field is immutable")
    {
        return true;
    }

    status.details.as_ref().is_some_and(|details| {
        details.causes.iter().any(|cause| {
            if cause.reason != "FieldValueForbidden" {
                return false;
            }
            let field = cause.field.trim_start_matches('.');
            let cause_message = cause.message.to_ascii_lowercase();
            field == "spec"
                || field.starts_with("spec.")
                || cause_message.contains("pod updates may not change fields")
                || cause_message.contains("field is immutable")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::core::{
        response::{StatusCause, StatusDetails},
        Status,
    };

    #[test]
    fn classifies_field_manager_conflict_errors() {
        let error = KubeError::Api(Box::new(Status {
            code: 409,
            details: Some(StatusDetails {
                name: String::new(),
                group: String::new(),
                kind: String::new(),
                uid: String::new(),
                causes: vec![StatusCause {
                    reason: "FieldManagerConflict".to_string(),
                    message: "conflict with \"helm\"".to_string(),
                    field: ".spec.replicas".to_string(),
                }],
                retry_after_seconds: 0,
            }),
            message: "Apply failed with conflicts".to_string(),
            reason: "Conflict".to_string(),
            ..Default::default()
        }));

        let app_error = apply_error(error);

        assert_eq!(app_error.kind, AppErrorKind::FieldManagerConflict);
        assert_eq!(app_error.message, "Apply failed with conflicts");
    }

    #[test]
    fn classifies_immutable_field_errors() {
        let message =
            "Forbidden: pod updates may not change fields other than spec.containers[*].image";
        let error = KubeError::Api(Box::new(Status {
            code: 422,
            details: Some(StatusDetails {
                name: String::new(),
                group: String::new(),
                kind: String::new(),
                uid: String::new(),
                causes: vec![StatusCause {
                    reason: "FieldValueForbidden".to_string(),
                    message: message.to_string(),
                    field: "spec".to_string(),
                }],
                retry_after_seconds: 0,
            }),
            message: message.to_string(),
            reason: "Invalid".to_string(),
            ..Default::default()
        }));

        let app_error = apply_error(error);

        assert_eq!(app_error.kind, AppErrorKind::ImmutableField);
        assert_eq!(app_error.message, message);
    }
}
