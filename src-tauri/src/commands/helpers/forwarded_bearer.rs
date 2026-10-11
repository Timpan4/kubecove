use super::session_scope::SessionScope;
use crate::models::{AppError, AppErrorKind};
use http::{header::AUTHORIZATION, HeaderValue};
use kube::client::Body;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use tower::{Layer, Service};

// The server's OIDC mode can send each Kubernetes request as the signed-in
// user instead of with the kubeconfig's credentials (ADR 0022 amendment).
// Each session scope owns a slot holding its user's bearer token, and every
// client built in that scope reads the slot per request: a refreshed token
// applies to cached clients and watch reconnects without rebuilding them.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turns forwarding on for the rest of the process. Kubeconfig credentials
/// are then never sent, and a client can only be built inside a session
/// scope.
pub fn enable_forwarded_bearer() {
    ENABLED.store(true, Ordering::SeqCst);
}

pub(crate) fn forwarded_bearer_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

#[derive(Default)]
pub(crate) struct BearerSlot(RwLock<Option<(HeaderValue, u64)>>);

impl BearerSlot {
    pub(crate) fn set(&self, token: Option<(&str, u64)>) {
        let value = token.and_then(|(token, expires_unix)| {
            let mut value = HeaderValue::from_str(&format!("Bearer {token}")).ok()?;
            value.set_sensitive(true);
            Some((value, expires_unix))
        });
        *self.0.write().expect("forwarded bearer lock") = value;
    }

    pub(crate) fn get(&self) -> Option<HeaderValue> {
        let mut slot = self.0.write().expect("forwarded bearer lock");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        if slot
            .as_ref()
            .is_some_and(|(_, expires)| !now.is_ok_and(|now| now.as_secs() < *expires))
        {
            *slot = None;
        }
        slot.as_ref().map(|(value, _)| value.clone())
    }
}

/// The layer for a client built now: the current scope's slot when
/// forwarding, and a pass-through otherwise.
pub(crate) fn forwarded_bearer_layer() -> Result<ForwardedBearerLayer, AppError> {
    layer_for(forwarded_bearer_enabled())
}

fn layer_for(enabled: bool) -> Result<ForwardedBearerLayer, AppError> {
    if !enabled {
        return Ok(ForwardedBearerLayer(None));
    }
    // Without a scope there is no user to act as. The kubeconfig's own
    // credentials are already stripped, so refuse instead of going anonymous.
    let scope = SessionScope::current().ok_or_else(|| {
        AppError::new(
            "Kubernetes request outside a signed-in session",
            AppErrorKind::Forbidden,
        )
    })?;
    Ok(ForwardedBearerLayer(Some(scope.bearer_slot())))
}

#[derive(Clone)]
pub(crate) struct ForwardedBearerLayer(Option<Arc<BearerSlot>>);

impl<S> Layer<S> for ForwardedBearerLayer {
    type Service = ForwardedBearerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ForwardedBearerService {
            inner,
            slot: self.0.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ForwardedBearerService<S> {
    inner: S,
    slot: Option<Arc<BearerSlot>>,
}

impl<S> Service<http::Request<Body>> for ForwardedBearerService<S>
where
    S: Service<http::Request<Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: http::Request<Body>) -> Self::Future {
        if let Some(slot) = &self.slot {
            let headers = request.headers_mut();
            headers.remove(AUTHORIZATION);
            if let Some(value) = slot.get() {
                headers.insert(AUTHORIZATION, value);
            }
        }
        self.inner.call(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Response};
    use tower::ServiceExt;

    async fn sent(layer: &ForwardedBearerLayer, incoming: Option<&str>) -> Option<String> {
        let (service, mut handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let mut service = layer.layer(service);
        let mut request = Request::builder().uri("/api");
        if let Some(value) = incoming {
            request = request.header(AUTHORIZATION, value);
        }
        let request = request.body(Body::empty()).unwrap();
        let call = tokio::spawn(async move { service.ready().await.unwrap().call(request).await });
        let (seen, reply) = handle.next_request().await.unwrap();
        reply.send_response(Response::new(Body::empty()));
        call.await.unwrap().unwrap();
        seen.headers()
            .get(AUTHORIZATION)
            .map(|value| value.to_str().unwrap().to_owned())
    }

    #[tokio::test]
    async fn a_slot_replaces_or_strips_the_authorization_header() {
        assert_eq!(
            sent(&ForwardedBearerLayer(None), Some("Bearer sa"))
                .await
                .as_deref(),
            Some("Bearer sa")
        );

        let slot = Arc::new(BearerSlot::default());
        let layer = ForwardedBearerLayer(Some(Arc::clone(&slot)));
        slot.set(Some(("user", u64::MAX)));
        assert_eq!(
            sent(&layer, Some("Bearer sa")).await.as_deref(),
            Some("Bearer user")
        );
        assert_eq!(sent(&layer, None).await.as_deref(), Some("Bearer user"));

        slot.set(None);
        assert_eq!(sent(&layer, Some("Bearer sa")).await, None);
    }

    // Not through ENABLED: it is process-wide and never turns off, and the
    // other tests in this process build clients outside any scope.
    #[tokio::test]
    async fn each_scope_sends_its_own_token_and_no_scope_builds_nothing() {
        assert_eq!(
            layer_for(true).err().map(|error| error.kind),
            Some(AppErrorKind::Forbidden)
        );

        let alice = SessionScope::new(None);
        let bob = SessionScope::new(None);
        alice.set_bearer(Some(("alice", u64::MAX)));
        bob.set_bearer(Some(("bob", u64::MAX)));
        let alice_layer = Arc::clone(&alice)
            .run(async { layer_for(true) })
            .await
            .unwrap();
        let bob_layer = Arc::clone(&bob)
            .run(async { layer_for(true) })
            .await
            .unwrap();
        let unscoped_in_spawn = Arc::clone(&alice)
            .run(async { tokio::spawn(async { layer_for(true).is_err() }).await })
            .await
            .unwrap();
        assert!(unscoped_in_spawn, "tokio::spawn does not inherit a scope");

        assert_eq!(
            sent(&alice_layer, None).await.as_deref(),
            Some("Bearer alice")
        );
        assert_eq!(sent(&bob_layer, None).await.as_deref(), Some("Bearer bob"));

        alice.set_bearer(Some(("alice-renewed", u64::MAX)));
        assert_eq!(
            sent(&alice_layer, None).await.as_deref(),
            Some("Bearer alice-renewed")
        );
        assert_eq!(sent(&bob_layer, None).await.as_deref(), Some("Bearer bob"));
    }
}
