//! Process-wide accounting of Kubernetes HTTP requests on the finite read path.
//!
//! A request interval runs from the send until its response body is fully read
//! or dropped. The counter adds up time while at least one interval is open, so
//! overlapping requests count once. GPUI reads snapshots before and after a load
//! to split the load's wall time into network time and processing time.

use bytes::Bytes;
use http::{Request, Response};
use http_body::{Body as HttpBody, Frame};
use kube::client::Body;
use std::{
    future::Future,
    io::Write,
    pin::Pin,
    sync::{Mutex, MutexGuard, PoisonError},
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::Instant;
use tower::{BoxError, Layer, Service};

/// The tracker every finite read client reports to.
pub static HTTP_ACTIVITY: HttpActivity = HttpActivity::new();

#[derive(Default)]
struct BusyState {
    in_flight: usize,
    busy_since: Option<Instant>,
    busy_total: Duration,
    requests: u64,
}

pub struct HttpActivity(Mutex<BusyState>);

/// A point in the monotonic busy clock. Compare two snapshots to measure a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HttpActivitySnapshot {
    busy: Duration,
    requests: u64,
}

impl HttpActivitySnapshot {
    /// Time with at least one request in flight between `earlier` and `self`.
    pub fn busy_since(self, earlier: Self) -> Duration {
        self.busy.saturating_sub(earlier.busy)
    }

    /// Requests sent between `earlier` and `self`.
    pub fn requests_since(self, earlier: Self) -> u64 {
        self.requests.saturating_sub(earlier.requests)
    }
}

impl HttpActivity {
    pub const fn new() -> Self {
        Self(Mutex::new(BusyState {
            in_flight: 0,
            busy_since: None,
            busy_total: Duration::ZERO,
            requests: 0,
        }))
    }

    pub fn snapshot(&self) -> HttpActivitySnapshot {
        let state = self.lock();
        let open = state
            .busy_since
            .map_or(Duration::ZERO, |since| since.elapsed());
        HttpActivitySnapshot {
            busy: state.busy_total + open,
            requests: state.requests,
        }
    }

    fn begin(&self) {
        let mut state = self.lock();
        if state.in_flight == 0 {
            state.busy_since = Some(Instant::now());
        }
        state.in_flight += 1;
        state.requests += 1;
    }

    fn end(&self) {
        let mut state = self.lock();
        state.in_flight = state.in_flight.saturating_sub(1);
        if state.in_flight == 0
            && let Some(since) = state.busy_since.take()
        {
            state.busy_total += since.elapsed();
        }
    }

    // A panic inside a request interval must not stop later intervals from closing.
    fn lock(&self) -> MutexGuard<'_, BusyState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for HttpActivity {
    fn default() -> Self {
        Self::new()
    }
}

/// One request interval. Dropping it closes the interval and logs the request, so
/// a cancelled future, an error or an abandoned body all end the interval.
struct Interval {
    activity: &'static HttpActivity,
    method: String,
    path: String,
    started: Instant,
    status: Option<u16>,
    bytes: u64,
}

impl Interval {
    fn begin(activity: &'static HttpActivity, method: String, path: String) -> Self {
        activity.begin();
        Self {
            activity,
            method,
            path,
            started: Instant::now(),
            status: None,
            bytes: 0,
        }
    }
}

impl Drop for Interval {
    fn drop(&mut self) {
        self.activity.end();
        let status = self
            .status
            .map_or_else(|| "none".to_owned(), |code| code.to_string());
        // Drop also runs on cancellation and unwinding, where a failed write
        // must not panic, so the error is discarded instead of `eprintln!`.
        let _ = writeln!(
            std::io::stderr(),
            "[kubecove:http] {} {} status={status} bytes={} ms={}",
            self.method,
            self.path,
            self.bytes,
            self.started.elapsed().as_millis()
        );
    }
}

/// Keeps the request interval open until the body is read to its end or dropped.
pub struct TrackedBody<B> {
    inner: B,
    interval: Option<Interval>,
}

impl<B> HttpBody for TrackedBody<B>
where
    B: HttpBody<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let poll = Pin::new(&mut this.inner).poll_frame(cx);
        match &poll {
            Poll::Ready(Some(Ok(frame))) => {
                if let (Some(data), Some(interval)) = (frame.data_ref(), this.interval.as_mut()) {
                    interval.bytes += data.len() as u64;
                }
            }
            Poll::Ready(None | Some(Err(_))) => {
                this.interval.take();
            }
            Poll::Pending => {}
        }
        poll
    }
}

#[derive(Clone)]
pub struct HttpActivityLayer {
    activity: &'static HttpActivity,
}

impl HttpActivityLayer {
    pub const fn new(activity: &'static HttpActivity) -> Self {
        Self { activity }
    }
}

impl<S> Layer<S> for HttpActivityLayer {
    type Service = HttpActivityService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpActivityService {
            inner,
            activity: self.activity,
        }
    }
}

#[derive(Clone)]
pub struct HttpActivityService<S> {
    inner: S,
    activity: &'static HttpActivity,
}

impl<S, B> Service<Request<Body>> for HttpActivityService<S>
where
    S: Service<Request<Body>, Response = Response<B>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: HttpBody<Data = Bytes> + Unpin + Send + 'static,
{
    type Response = Response<TrackedBody<B>>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let activity = self.activity;
        let method = request.method().as_str().to_owned();
        let path = request.uri().path().to_owned();
        let future = self.inner.call(request);
        Box::pin(async move {
            let mut interval = Interval::begin(activity, method, path);
            let response = future.await.map_err(Into::into)?;
            interval.status = Some(response.status().as_u16());
            Ok(response.map(|inner| TrackedBody {
                inner,
                interval: Some(interval),
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::{pending, poll_fn};
    use tokio::time::{sleep, timeout};
    use tower::service_fn;

    // Failure modes, written before the tests:
    // 1. Overlapping requests count their shared time once.
    // 2. The response body keeps its interval open until it is read to the end.
    // 3. A request future dropped before its response closes its interval.
    // 4. A body dropped before its end closes its interval.
    // 5. An inner error closes its interval.
    // Each test runs on a paused clock, so the busy time is exact.

    /// A body that yields one frame after `delay`, then ends.
    struct DelayedBody {
        delay: Pin<Box<tokio::time::Sleep>>,
        data: Option<Bytes>,
    }

    impl DelayedBody {
        fn new(delay: Duration, data: Option<Bytes>) -> Self {
            Self {
                delay: Box::pin(sleep(delay)),
                data,
            }
        }
    }

    impl HttpBody for DelayedBody {
        type Data = Bytes;
        type Error = BoxError;

        fn poll_frame(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
            if self.delay.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            Poll::Ready(self.data.take().map(|data| Ok(Frame::data(data))))
        }
    }

    fn tracker() -> &'static HttpActivity {
        // Each test gets its own tracker, so the process-wide one never mixes in.
        Box::leak(Box::new(HttpActivity::new()))
    }

    async fn drain<B>(mut body: B)
    where
        B: HttpBody<Data = Bytes> + Unpin,
    {
        while poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .is_some()
        {}
    }

    fn request() -> Request<Body> {
        Request::new(Body::empty())
    }

    #[tokio::test(start_paused = true)]
    async fn overlapping_requests_count_shared_time_once() {
        let activity = tracker();
        let make = || {
            HttpActivityLayer::new(activity).layer(service_fn(|_: Request<Body>| async {
                sleep(Duration::from_millis(10)).await;
                Ok::<_, BoxError>(Response::new(DelayedBody::new(Duration::ZERO, None)))
            }))
        };
        let (mut first, mut second) = (make(), make());
        let start = activity.snapshot();

        let (a, b) = tokio::join!(first.call(request()), second.call(request()));
        drain(a.expect("first response")).await;
        drain(b.expect("second response")).await;

        let end = activity.snapshot();
        assert_eq!(end.busy_since(start), Duration::from_millis(10));
        assert_eq!(end.requests_since(start), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn body_download_keeps_interval_open() {
        let activity = tracker();
        let mut service =
            HttpActivityLayer::new(activity).layer(service_fn(|_: Request<Body>| async {
                Ok::<_, BoxError>(Response::new(DelayedBody::new(
                    Duration::from_millis(20),
                    Some(Bytes::from_static(b"{}")),
                )))
            }));
        let start = activity.snapshot();

        let response = service.call(request()).await.expect("response");
        sleep(Duration::from_millis(5)).await;
        drain(response.into_body()).await;

        let end = activity.snapshot();
        assert_eq!(end.busy_since(start), Duration::from_millis(20));
        assert_eq!(end.requests_since(start), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_request_future_closes_interval() {
        let activity = tracker();
        let mut service =
            HttpActivityLayer::new(activity).layer(service_fn(|_: Request<Body>| async {
                pending::<()>().await;
                Ok::<_, BoxError>(Response::new(DelayedBody::new(Duration::ZERO, None)))
            }));
        let start = activity.snapshot();

        let timed_out = timeout(Duration::from_millis(5), service.call(request())).await;
        assert!(timed_out.is_err());
        let closed = activity.snapshot();
        sleep(Duration::from_millis(100)).await;

        let later = activity.snapshot();
        assert_eq!(closed.busy_since(start), Duration::from_millis(5));
        assert_eq!(later.busy_since(closed), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_body_closes_interval() {
        let activity = tracker();
        let mut service =
            HttpActivityLayer::new(activity).layer(service_fn(|_: Request<Body>| async {
                Ok::<_, BoxError>(Response::new(DelayedBody::new(
                    Duration::from_secs(3600),
                    Some(Bytes::from_static(b"{}")),
                )))
            }));
        let start = activity.snapshot();

        let response = service.call(request()).await.expect("response");
        let timed_out = timeout(Duration::from_millis(5), drain(response.into_body())).await;
        assert!(timed_out.is_err());
        let closed = activity.snapshot();
        sleep(Duration::from_millis(100)).await;

        let later = activity.snapshot();
        assert_eq!(closed.busy_since(start), Duration::from_millis(5));
        assert_eq!(later.busy_since(closed), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn inner_error_closes_interval() {
        let activity = tracker();
        let mut service =
            HttpActivityLayer::new(activity).layer(service_fn(|_: Request<Body>| async {
                sleep(Duration::from_millis(3)).await;
                Err::<Response<DelayedBody>, BoxError>("connection reset".into())
            }));
        let start = activity.snapshot();

        let result = service.call(request()).await;
        assert!(result.is_err());
        let closed = activity.snapshot();
        sleep(Duration::from_millis(100)).await;

        let later = activity.snapshot();
        assert_eq!(closed.busy_since(start), Duration::from_millis(3));
        assert_eq!(later.busy_since(closed), Duration::ZERO);
    }
}
