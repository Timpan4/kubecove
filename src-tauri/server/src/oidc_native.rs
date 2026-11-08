//! Desktop sign-in through the system browser (ADR 0031). The desktop is a
//! public native client of this server: it receives a single-use code on a
//! loopback redirect and exchanges it, with its PKCE verifier, for an opaque
//! sign-in credential that it then sends as a bearer.
use super::{
    Credential, Oidc, TRANSACTION_COOKIE, clear_cookie_header, cookie_header, cookie_value,
    oidc_unavailable, transaction_cookie, unix_now, verify_transaction,
};
use crate::{
    ApiError,
    observe::{self, AuthMode, SignInFailure},
    security::{equal_secret, random_token},
};
use axum::{
    Json,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use kubecove_lib::models::AppErrorKind;
use openidconnect::{PkceCodeChallenge, PkceCodeVerifier, url::Url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const CALLBACK_PATH: &str = "/kubecove/callback";

/// The desktop's loopback redirect, PKCE challenge and state. It travels in
/// the signed transaction cookie from the start request to the callback.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct NativeRequest {
    redirect_uri: String,
    challenge: String,
    state: String,
}

#[derive(Deserialize)]
pub struct NativeStart {
    redirect_uri: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeToken {
    code: String,
    code_verifier: String,
    redirect_uri: String,
}

struct PendingCode {
    credential: String,
    redirect_uri: String,
    challenge: String,
    deadline: u64,
}

#[derive(Default)]
pub(super) struct NativeCodes {
    pending: HashMap<String, PendingCode>,
    /// Exchanged codes and the credential each produced, kept until the code's
    /// deadline so a replay can revoke that credential (RFC 9700 4.2.4).
    consumed: HashMap<String, (String, u64)>,
}

impl NativeStart {
    fn validate(self) -> Option<NativeRequest> {
        let redirect_uri = self.redirect_uri.filter(|uri| is_loopback_redirect(uri))?;
        if self.code_challenge_method.as_deref() != Some("S256") {
            return None;
        }
        let challenge = self
            .code_challenge
            .filter(|value| is_encoded_32_bytes(value))?;
        let state = self.state.filter(|value| is_encoded_32_bytes(value))?;
        Some(NativeRequest {
            redirect_uri,
            challenge,
            state,
        })
    }
}

/// `http://127.0.0.1:<port>/kubecove/callback` or the `[::1]` form, with a
/// non-zero port written without sign or leading zeros (RFC 8252 7.3).
fn is_loopback_redirect(uri: &str) -> bool {
    let Some(rest) = uri
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| uri.strip_prefix("http://[::1]:"))
    else {
        return false;
    };
    let Some(port) = rest.strip_suffix(CALLBACK_PATH) else {
        return false;
    };
    port.parse::<u16>()
        .is_ok_and(|number| number != 0 && number.to_string() == port)
}

/// Unpadded base64url of 32 bytes: an S256 challenge (RFC 7636 4.2), and the
/// desktop's state, which it draws as 32 bytes from the OS CSPRNG.
fn is_encoded_32_bytes(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// RFC 7636 4.1: 43 to 128 unreserved characters.
fn is_valid_verifier(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'))
}

impl Oidc {
    pub fn native_start(&self, request: NativeStart) -> Result<Response, ApiError> {
        let request = request.validate().ok_or_else(invalid_start)?;
        let (auth_url, signed) = self.begin(Some(request))?;
        let mut response = Redirect::to(auth_url.as_str()).into_response();
        response.headers_mut().append(
            header::SET_COOKIE,
            cookie_header(&transaction_cookie(&signed))?,
        );
        Ok(response)
    }

    /// Hands a verified native sign-in back to the desktop's loopback listener.
    pub(super) fn issue_code(
        &self,
        credential: &str,
        request: NativeRequest,
        deadline: u64,
    ) -> Result<Response, ApiError> {
        let code = random_token().map_err(|_| oidc_unavailable())?;
        let mut redirect = Url::parse(&request.redirect_uri).map_err(|_| oidc_unavailable())?;
        redirect
            .query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &request.state);
        self.codes
            .lock()
            .map_err(|_| oidc_unavailable())?
            .pending
            .insert(
                code,
                PendingCode {
                    credential: credential.to_owned(),
                    redirect_uri: request.redirect_uri,
                    challenge: request.challenge,
                    deadline,
                },
            );
        Ok(Redirect::to(redirect.as_str()).into_response())
    }

    /// Sends a provider denial to the desktop as `error=access_denied`, for a
    /// native transaction whose provider state matches.
    pub(super) fn native_denial(
        &self,
        headers: &HeaderMap,
        state: Option<&str>,
    ) -> Option<Response> {
        let signed = cookie_value(headers, TRANSACTION_COOKIE)?;
        let transaction = verify_transaction(signed, &self.transaction_key, unix_now()).ok()?;
        let native = transaction.native?;
        if !equal_secret(state?, &transaction.state) {
            return None;
        }
        let mut redirect = Url::parse(&native.redirect_uri).ok()?;
        redirect
            .query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &native.state);
        let mut response = Redirect::to(redirect.as_str()).into_response();
        response.headers_mut().append(
            header::SET_COOKIE,
            cookie_header(&clear_cookie_header(TRANSACTION_COOKIE)).ok()?,
        );
        Some(response)
    }

    pub fn native_token(&self, request: NativeToken) -> Result<Response, ApiError> {
        let mut credentials = self.credentials.lock().map_err(|_| oidc_unavailable())?;
        self.prune_expired(&mut credentials);
        let mut codes = self.codes.lock().map_err(|_| oidc_unavailable())?;
        // Removed before any check, so a code is presented at most once.
        let Some(pending) = codes.pending.remove(&request.code) else {
            let replayed = codes.consumed.remove(&request.code);
            drop(codes);
            if let Some((credential, _)) = replayed {
                self.revoke_entry(&mut credentials, &credential);
            }
            return Err(refused_code());
        };
        let verifier_matches = is_valid_verifier(&request.code_verifier) && {
            let challenge = PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
                request.code_verifier,
            ));
            equal_secret(challenge.as_str(), &pending.challenge)
        };
        if pending.deadline <= unix_now()
            || request.redirect_uri != pending.redirect_uri
            || !verifier_matches
            || !credentials.contains_key(&pending.credential)
        {
            drop(codes);
            // Nobody else can ever obtain this credential.
            self.revoke_entry(&mut credentials, &pending.credential);
            return Err(refused_code());
        }
        codes
            .consumed
            .insert(request.code, (pending.credential.clone(), pending.deadline));
        Ok(Json(serde_json::json!({ "credential": pending.credential })).into_response())
    }

    /// Drops expired codes and returns the credentials of codes that were
    /// never exchanged, which the caller revokes.
    pub(super) fn prune_codes(&self, now: u64) -> Vec<String> {
        let Ok(mut codes) = self.codes.lock() else {
            return Vec::new();
        };
        codes.consumed.retain(|_, (_, deadline)| *deadline > now);
        let expired = codes
            .pending
            .iter()
            .filter(|(_, code)| code.deadline <= now)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        expired
            .into_iter()
            .filter_map(|key| codes.pending.remove(&key))
            .map(|code| code.credential)
            .collect()
    }

    fn revoke_entry(&self, credentials: &mut HashMap<String, Credential>, key: &str) {
        if let Some(credential) = credentials.remove(key) {
            credential.revoked.cancel();
        }
        self.deactivate(key);
    }
}

fn invalid_start() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "desktop sign-in request is invalid",
        AppErrorKind::Validation,
    )
}

fn refused_code() -> ApiError {
    observe::sign_in_failed(AuthMode::Oidc, SignInFailure::InvalidCallback);
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "desktop sign-in code is invalid or expired",
        AppErrorKind::Forbidden,
    )
}

#[cfg(test)]
#[path = "oidc_native_tests.rs"]
mod tests;
