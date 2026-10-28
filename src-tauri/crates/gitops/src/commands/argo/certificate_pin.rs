//! Certificate pinning for Argo CD servers reached through a private Service tunnel.
//!
//! A tunnel target is identified by its Kubernetes namespace and Service, not by a public CA,
//! so HTTPS through a tunnel accepts exactly the certificate the user confirmed (ADR 0017).

use crate::models::{AppError, AppErrorKind};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider},
    pki_types::{CertificateDer, ServerName, UnixTime},
    CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme,
};
use std::sync::{Arc, Mutex};

/// SHA-256 of a DER certificate as lowercase hex.
pub(crate) fn certificate_sha256(der: &[u8]) -> String {
    use std::fmt::Write;
    let digest = ring::digest::digest(&ring::digest::SHA256, der);
    let mut hex = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Accepts lowercase or uppercase hex, with or without `:` separators.
pub(crate) fn parse_certificate_sha256(value: &str) -> Result<String, AppError> {
    let normalized = value
        .trim()
        .chars()
        .filter(|character| *character != ':')
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized.len() == 64 && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(normalized);
    }
    Err(AppError::new(
        "invalid Argo CD certificate fingerprint",
        AppErrorKind::ArgoConnection,
    ))
}

/// The certificate a server presented during the last handshake, if any.
#[derive(Clone, Default)]
pub(crate) struct PresentedCertificate(Arc<Mutex<Option<String>>>);

impl PresentedCertificate {
    pub(crate) fn get(&self) -> Option<String> {
        self.0.lock().ok().and_then(|presented| presented.clone())
    }

    fn record(&self, fingerprint: String) {
        if let Ok(mut presented) = self.0.lock() {
            *presented = Some(fingerprint);
        }
    }
}

impl std::fmt::Debug for PresentedCertificate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("PresentedCertificate")
            .field(&self.get())
            .finish()
    }
}

/// TLS configuration that accepts only `pinned`. Without a pin every certificate is rejected
/// and only recorded, so a probe never completes a handshake that could carry credentials.
pub(crate) fn pinned_tls_config(
    pinned: Option<String>,
) -> Result<(ClientConfig, PresentedCertificate), AppError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let presented = PresentedCertificate::default();
    let verifier = PinnedCertificateVerifier {
        pinned,
        presented: presented.clone(),
        provider: provider.clone(),
    };
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| {
            AppError::new(
                "TLS configuration unavailable",
                AppErrorKind::ArgoConnection,
            )
            .with_source(error)
        })?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok((config, presented))
}

#[derive(Debug)]
struct PinnedCertificateVerifier {
    pinned: Option<String>,
    presented: PresentedCertificate,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = certificate_sha256(end_entity.as_ref());
        let accepted = self.pinned.as_deref() == Some(fingerprint.as_str());
        self.presented.record(fingerprint);
        if accepted {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Failure modes: an unpinned handshake succeeds; a different certificate is accepted; the
    // pinned certificate is rejected; separator or case differences reject a valid pin; a
    // malformed fingerprint is accepted as a pin.

    fn verify(pinned: Option<&str>, der: &[u8]) -> (bool, Option<String>) {
        let presented = PresentedCertificate::default();
        let verifier = PinnedCertificateVerifier {
            pinned: pinned.map(str::to_owned),
            presented: presented.clone(),
            provider: Arc::new(rustls::crypto::ring::default_provider()),
        };
        let accepted = verifier
            .verify_server_cert(
                &CertificateDer::from(der.to_vec()),
                &[],
                &ServerName::try_from("argocd-server.argocd.svc").unwrap(),
                &[],
                UnixTime::now(),
            )
            .is_ok();
        (accepted, presented.get())
    }

    #[test]
    fn only_the_pinned_certificate_is_accepted() {
        let server = certificate_sha256(b"server");
        assert_eq!(verify(None, b"server"), (false, Some(server.clone())));
        assert_eq!(
            verify(Some(&server), b"server"),
            (true, Some(server.clone()))
        );
        assert_eq!(
            verify(Some(&server), b"impostor"),
            (false, Some(certificate_sha256(b"impostor")))
        );
    }

    #[test]
    fn fingerprints_normalize_separators_and_case() {
        let fingerprint = certificate_sha256(b"server");
        let display = fingerprint
            .to_ascii_uppercase()
            .as_bytes()
            .chunks(2)
            .map(|pair| std::str::from_utf8(pair).unwrap())
            .collect::<Vec<_>>()
            .join(":");
        assert_eq!(parse_certificate_sha256(&display).unwrap(), fingerprint);
        assert!(parse_certificate_sha256("abc").is_err());
        assert!(parse_certificate_sha256(&"g".repeat(64)).is_err());
    }
}
