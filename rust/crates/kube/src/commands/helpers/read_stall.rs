//! Fails a request in flight when the server stalls: no response headers, or
//! no next body frame, within the limit. This is ADR 0009's 30-second fallback
//! for stalled endpoints. It used to be the socket read timeout, which also
//! closed every idle pooled connection 30 s after its last use, so each later
//! burst of reads paid one TLS handshake per request.

use bytes::Bytes;
use http::{Request, Response};
use http_body::{Body as HttpBody, Frame};
use kube::client::Body;
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::{Instant, Sleep, sleep};
use tower::{BoxError, Layer, Service};

fn stalled() -> BoxError {
    Box::new(io::Error::new(
        io::ErrorKind::TimedOut,
        "Kubernetes API server stopped responding",
    ))
}

#[derive(Clone)]
pub(crate) struct ReadStallLayer {
    limit: Duration,
}

impl ReadStallLayer {
    pub(crate) const fn new(limit: Duration) -> Self {
        Self { limit }
    }
}

impl<S> Layer<S> for ReadStallLayer {
    type Service = ReadStallService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ReadStallService {
            inner,
            limit: self.limit,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ReadStallService<S> {
    inner: S,
    limit: Duration,
}

impl<S, B> Service<Request<Body>> for ReadStallService<S>
where
    S: Service<Request<Body>, Response = Response<B>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: HttpBody<Data = Bytes> + Unpin + Send + 'static,
    B::Error: Into<BoxError>,
{
    type Response = Response<StallBody<B>>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let limit = self.limit;
        let future = self.inner.call(request);
        Box::pin(async move {
            let response = tokio::time::timeout(limit, future)
                .await
                .map_err(|_| stalled())?
                .map_err(Into::into)?;
            Ok(response.map(|inner| StallBody {
                inner,
                limit,
                deadline: Box::pin(sleep(limit)),
                armed: false,
            }))
        })
    }
}

/// Arms the deadline only while a frame is awaited, as the socket timeout
/// did, so a caller that reads the body slowly is never failed for it.
pub(crate) struct StallBody<B> {
    inner: B,
    limit: Duration,
    deadline: Pin<Box<Sleep>>,
    armed: bool,
}

impl<B> HttpBody for StallBody<B>
where
    B: HttpBody<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if let Poll::Ready(frame) = Pin::new(&mut this.inner).poll_frame(cx) {
            this.armed = false;
            return Poll::Ready(frame.map(|frame| frame.map_err(Into::into)));
        }
        if !this.armed {
            this.deadline.as_mut().reset(Instant::now() + this.limit);
            this.armed = true;
        }
        match this.deadline.as_mut().poll(cx) {
            Poll::Ready(()) => Poll::Ready(Some(Err(stalled()))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::pending;
    use tower::{ServiceExt, service_fn};

    // Failure modes, written before the tests:
    // 1. Headers that never arrive hang the read instead of failing after the limit.
    // 2. A body that stops between frames hangs instead of failing after the limit.
    // 3. A caller that reads the body slowly fails although the server kept up.
    // 4. A long download whose frames each arrive in time fails on total duration.
    // 5. A stall error stops classifying as a network failure.

    const LIMIT: Duration = Duration::from_secs(30);

    /// Yields one frame per delay, then ends; a `None` delay never arrives.
    struct PacedBody(Vec<Option<Duration>>, Option<Pin<Box<Sleep>>>);

    impl HttpBody for PacedBody {
        type Data = Bytes;
        type Error = BoxError;

        fn poll_frame(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
            let Some(next) = self.0.first().copied() else {
                return Poll::Ready(None);
            };
            let Some(delay) = next else {
                return Poll::Pending;
            };
            let wait = self.1.get_or_insert_with(|| Box::pin(sleep(delay)));
            if wait.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.1 = None;
            self.0.remove(0);
            Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"x")))))
        }
    }

    fn request() -> Request<Body> {
        Request::get("/api/v1/pods").body(Body::empty()).unwrap()
    }

    async fn respond(frames: Vec<Option<Duration>>) -> Response<StallBody<PacedBody>> {
        let service = service_fn(move |_| {
            let frames = frames.clone();
            async move { Ok::<_, BoxError>(Response::new(PacedBody(frames, None))) }
        });
        ReadStallLayer::new(LIMIT)
            .layer(service)
            .oneshot(request())
            .await
            .unwrap()
    }

    async fn drain(body: &mut StallBody<PacedBody>) -> Result<usize, BoxError> {
        let mut frames = 0;
        while let Some(frame) = std::future::poll_fn(|cx| Pin::new(&mut *body).poll_frame(cx)).await
        {
            frame?;
            frames += 1;
        }
        Ok(frames)
    }

    fn is_timeout(error: &BoxError) -> bool {
        error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::TimedOut)
    }

    #[tokio::test(start_paused = true)]
    async fn missing_headers_fail_after_the_limit() {
        let service = service_fn(|_| pending::<Result<Response<PacedBody>, BoxError>>());
        let started = Instant::now();
        let error = ReadStallLayer::new(LIMIT)
            .layer(service)
            .oneshot(request())
            .await
            .err()
            .expect("stalled headers fail");
        assert!(is_timeout(&error));
        assert_eq!(started.elapsed(), LIMIT);
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_body_fails_after_the_limit() {
        let mut body = respond(vec![Some(Duration::from_secs(1)), None])
            .await
            .into_body();
        let started = Instant::now();
        let error = drain(&mut body).await.expect_err("stalled body fails");
        assert!(is_timeout(&error));
        assert_eq!(started.elapsed(), Duration::from_secs(1) + LIMIT);
    }

    #[tokio::test(start_paused = true)]
    async fn slow_reader_and_long_download_both_succeed() {
        let frame_gap = Duration::from_secs(29);
        let mut body = respond(vec![Some(frame_gap); 3]).await.into_body();
        let first = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await;
        assert!(first.is_some_and(|frame| frame.is_ok()));
        // The caller pauses longer than the limit before reading on.
        tokio::time::advance(LIMIT * 2).await;
        assert_eq!(drain(&mut body).await.unwrap(), 2);
    }
}
