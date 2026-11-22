use super::*;

#[test]
fn only_the_response_cap_reads_as_too_large() {
    let mut bytes = vec![0; MAX_RESPONSE_BYTES];
    let capped = push_capped_chunk(&mut bytes, &[0]).unwrap_err();
    assert!(is_response_too_large(&capped));

    let denied = AppError::new(
        "Argo CD API request failed (403 Forbidden)",
        AppErrorKind::ArgoApi,
    );
    assert!(!is_response_too_large(&denied));
}
