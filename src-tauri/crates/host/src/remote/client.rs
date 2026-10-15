use super::models::{RemoteConnectionSummary, RemoteCredentials, ServerSession};
use super::tunnels::RemoteTunnelRegistry;
use futures_util::{SinkExt, StreamExt};
use kubecove_kube::runtime::Channel;
use kubecove_models::{AppError, AppErrorKind};
use reqwest::{header::HeaderValue, Client as HttpClient, Url};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, RwLock},
    task::JoinHandle,
    time::Instant,
};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{
        client::IntoClientRequest, http::header::AUTHORIZATION, protocol::WebSocketConfig,
        Message as WsMessage,
    },
};

#[cfg(test)]
const CHANNEL_PREFIX: &str = "__KUBECOVE_CHANNEL__:";
const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_TUNNEL_PAYLOAD_BYTES: usize = 16 * 1024;

pub struct RemoteConnectionStore {
    pub(super) current: RwLock<Option<Arc<RemoteConnection>>>,
}

impl Default for RemoteConnectionStore {
    fn default() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }
}

pub struct RemoteConnection {
    pub(super) credentials: RemoteCredentials,
    pub(super) http: HttpClient,
    pub(super) summary: RemoteConnectionSummary,
    outgoing: mpsc::Sender<WsMessage>,
    channels: Mutex<HashMap<u64, Channel<Value>>>,
    controls: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    pub(super) tunnel_senders: Mutex<HashMap<String, mpsc::Sender<Vec<u8>>>>,
    pub(super) tunnels: Arc<RemoteTunnelRegistry>,
    active: AtomicBool,
    socket_task: Mutex<Option<JoinHandle<()>>>,
}

impl RemoteConnection {
    pub async fn invoke(&self, command: &str, args: Value) -> Result<Value, AppError> {
        let url = api_url(&self.credentials.server_url, &format!("invoke/{command}"))?;
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.credentials.bearer_token)
            .header("X-Kubecove-Session", &self.credentials.resume_token)
            .json(&args)
            .send()
            .await
            .map_err(network_error)?;
        response_value(response).await
    }

    pub fn subscribe(&self, channel_id: u64, channel: Channel<Value>) -> Result<(), AppError> {
        if channel_id > (1_u64 << 53) - 1 {
            return Err(validation_error(
                "channel id is outside the supported range",
            ));
        }
        let mut channels = self.channels.lock().expect("remote channel registry lock");
        if channels.contains_key(&channel_id) {
            return Err(validation_error("channel id is already subscribed"));
        }
        channels.insert(channel_id, channel);
        Ok(())
    }

    pub async fn unsubscribe(&self, channel_id: u64) -> Result<bool, AppError> {
        let removed = self
            .channels
            .lock()
            .expect("remote channel registry lock")
            .remove(&channel_id)
            .is_some();
        let url = api_url(
            &self.credentials.server_url,
            &format!("channels/{channel_id}/close"),
        )?;
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.credentials.bearer_token)
            .header("X-Kubecove-Session", &self.credentials.resume_token)
            .send()
            .await
            .map_err(network_error)?;
        response_value(response).await?;
        Ok(removed)
    }

    pub async fn send_control(&self, value: Value, key: String) -> Result<Value, AppError> {
        if !self.active.load(Ordering::Acquire) {
            return Err(session_error("remote server session is disconnected"));
        }
        let (sender, receiver) = oneshot::channel();
        self.controls
            .lock()
            .expect("remote control registry lock")
            .insert(key.clone(), sender);
        if self
            .outgoing
            .send(WsMessage::Text(value.to_string().into()))
            .await
            .is_err()
        {
            self.controls
                .lock()
                .expect("remote control registry lock")
                .remove(&key);
            return Err(session_error("remote server WebSocket is closed"));
        }
        let response = receiver
            .await
            .map_err(|_| session_error("remote server WebSocket is closed"))?;
        if response.get("type").and_then(Value::as_str) == Some("tunnel_error") {
            return Err(serde_json::from_value(
                response.get("error").cloned().unwrap_or(Value::Null),
            )
            .unwrap_or_else(|_| session_error("remote tunnel request failed")));
        }
        Ok(response)
    }

    pub async fn send_control_without_reply(&self, value: Value) -> Result<(), AppError> {
        if !self.active.load(Ordering::Acquire) {
            return Err(session_error("remote server session is disconnected"));
        }
        self.outgoing
            .send(WsMessage::Text(value.to_string().into()))
            .await
            .map_err(|_| session_error("remote server WebSocket is closed"))
    }

    pub async fn send_binary(&self, connection_id: &str, payload: &[u8]) -> Result<(), AppError> {
        if payload.len() > MAX_TUNNEL_PAYLOAD_BYTES {
            return Err(validation_error("tunnel payload exceeds 16 KiB"));
        }
        if !is_canonical_uuid(connection_id) {
            return Err(validation_error("connection id must be a canonical UUID"));
        }
        let mut frame = Vec::with_capacity(2 + connection_id.len() + payload.len());
        frame.extend_from_slice(&(connection_id.len() as u16).to_be_bytes());
        frame.extend_from_slice(connection_id.as_bytes());
        frame.extend_from_slice(payload);
        self.outgoing
            .send(WsMessage::Binary(frame.into()))
            .await
            .map_err(|_| session_error("remote server WebSocket is closed"))
    }

    pub fn add_tunnel_sender(
        &self,
        connection_id: String,
        sender: mpsc::Sender<Vec<u8>>,
    ) -> Result<(), AppError> {
        if !is_canonical_uuid(&connection_id) {
            return Err(validation_error("connection id must be a canonical UUID"));
        }
        let mut senders = self
            .tunnel_senders
            .lock()
            .expect("remote tunnel connection lock");
        if senders.contains_key(&connection_id) {
            return Err(session_error("tunnel connection id is already active"));
        }
        senders.insert(connection_id, sender);
        Ok(())
    }

    pub fn remove_tunnel_sender(&self, connection_id: &str) {
        self.tunnel_senders
            .lock()
            .expect("remote tunnel connection lock")
            .remove(connection_id);
    }

    pub async fn close(&self) {
        self.active.store(false, Ordering::Release);
        if let Some(task) = self
            .socket_task
            .lock()
            .expect("remote socket task lock")
            .take()
        {
            task.abort();
        }
        self.fail_controls();
        self.channels
            .lock()
            .expect("remote channel registry lock")
            .clear();
        self.tunnel_senders
            .lock()
            .expect("remote tunnel connection lock")
            .clear();
        self.tunnels.close_all(None).await;

        let http = self.http.clone();
        let server_url = self.credentials.server_url.clone();
        let bearer_token = self.credentials.bearer_token.clone();
        let resume_token = self.credentials.resume_token.clone();
        let grace = Duration::from_secs(self.summary.session_grace_seconds);
        tokio::spawn(async move {
            let close_result = async {
                let url = api_url(&server_url, "session/close")?;
                let response = http
                    .post(url)
                    .bearer_auth(&bearer_token)
                    .header("X-Kubecove-Session", &resume_token)
                    .send()
                    .await
                    .map_err(network_error)?;
                response_value(response).await.map(|_| ())
            };
            if let Err(error) = tokio::time::timeout(grace, close_result)
                .await
                .unwrap_or_else(|_| Err(session_error("remote server session close timed out")))
            {
                eprintln!(
                    "remote server session close failed: {}",
                    error.kind.as_str()
                );
            }
        });
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn fail_controls(&self) {
        self.controls
            .lock()
            .expect("remote control registry lock")
            .clear();
    }
}

pub async fn connect(
    server_url: &str,
    bearer_token: &str,
    resume_token: Option<&str>,
) -> Result<Arc<RemoteConnection>, AppError> {
    let server_url = normalize_server_url(server_url)?;
    let bearer_token = bearer_token.trim();
    if bearer_token.is_empty() {
        return Err(validation_error("server token is required"));
    }
    install_tls_provider();
    let http = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(network_error)?;
    let session_url = api_url(&server_url, "session")?;
    let mut login = json!({});
    if let Some(resume_token) = resume_token {
        login["resumeToken"] = Value::String(resume_token.to_string());
    }
    let response = http
        .post(session_url)
        .bearer_auth(bearer_token)
        .json(&login)
        .send()
        .await
        .map_err(network_error)?;
    let session: ServerSession = response_json(response).await?;
    let credentials = RemoteCredentials {
        server_url,
        bearer_token: bearer_token.to_string(),
        resume_token: session.resume_token,
    };
    let summary = RemoteConnectionSummary {
        server_url: credentials.server_url.clone(),
        server_version: session.server_version,
        session_grace_seconds: session.session_grace_seconds,
        connected: true,
    };
    let socket = connect_websocket(&credentials).await?;
    let (outgoing, receiver) = mpsc::channel(32);
    let connection = Arc::new(RemoteConnection {
        credentials,
        http,
        summary,
        outgoing,
        channels: Mutex::new(HashMap::new()),
        controls: Mutex::new(HashMap::new()),
        tunnel_senders: Mutex::new(HashMap::new()),
        tunnels: Arc::new(RemoteTunnelRegistry::default()),
        active: AtomicBool::new(true),
        socket_task: Mutex::new(None),
    });
    let connection_for_task = Arc::clone(&connection);
    let task = tokio::spawn(async move {
        run_socket(connection_for_task, socket, receiver).await;
    });
    *connection
        .socket_task
        .lock()
        .expect("remote socket task lock") = Some(task);
    Ok(connection)
}

async fn connect_websocket(
    credentials: &RemoteCredentials,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    AppError,
> {
    let mut url = api_url(&credentials.server_url, "channels")?;
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    url.set_scheme(scheme)
        .map_err(|()| validation_error("server URL has an unsupported scheme"))?;
    url.query_pairs_mut()
        .append_pair("resumeToken", &credentials.resume_token);
    let mut request = url.as_str().into_client_request().map_err(|error| {
        AppError::new(
            "remote WebSocket request is invalid",
            AppErrorKind::Validation,
        )
        .with_source(error)
    })?;
    let bearer = HeaderValue::from_str(&format!("Bearer {}", credentials.bearer_token)).map_err(
        |error| {
            AppError::new(
                "server token is invalid for HTTP authorization",
                AppErrorKind::Validation,
            )
            .with_source(error)
        },
    )?;
    request.headers_mut().insert(AUTHORIZATION, bearer);
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(MAX_WEBSOCKET_MESSAGE_BYTES);
    config.max_frame_size = Some(MAX_WEBSOCKET_MESSAGE_BYTES);
    connect_async_with_config(request, Some(config), false)
        .await
        .map(|(socket, _)| socket)
        .map_err(|error| {
            AppError::new(
                "could not connect to the remote server WebSocket",
                AppErrorKind::Network,
            )
            .with_source(error)
        })
}

async fn run_socket(
    connection: Arc<RemoteConnection>,
    mut socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    mut receiver: mpsc::Receiver<WsMessage>,
) {
    let grace = Duration::from_secs(connection.summary.session_grace_seconds);
    loop {
        let (mut sink, mut stream) = socket.split();
        let mut writer = Box::pin(async {
            while let Some(message) = receiver.recv().await {
                sink.send(message).await.map_err(|_| ())?;
            }
            Ok::<(), ()>(())
        });
        let mut reader = Box::pin(async {
            while let Some(incoming) = stream.next().await {
                match incoming {
                    Ok(WsMessage::Text(text)) if handle_text(&connection, text.as_str()) => {
                        return true;
                    }
                    Ok(WsMessage::Binary(bytes)) => handle_binary(&connection, &bytes).await,
                    Ok(WsMessage::Ping(bytes)) => {
                        if connection
                            .outgoing
                            .try_send(WsMessage::Pong(bytes))
                            .is_err()
                        {
                            return false;
                        }
                    }
                    Ok(WsMessage::Close(_)) | Err(_) => return false,
                    _ => {}
                }
            }
            false
        });
        let moved = tokio::select! {
            writer_result = &mut writer => {
                if writer_result.is_ok() {
                    return;
                }
                false
            }
            moved = &mut reader => moved,
        };
        drop(writer);
        drop(reader);

        if moved {
            connection.active.store(false, Ordering::Release);
            connection.fail_controls();
            connection
                .tunnel_senders
                .lock()
                .expect("remote tunnel connection lock")
                .clear();
            connection.tunnels.close_all(None).await;
            return;
        }

        let disconnected_at = Instant::now();
        let reconnect_deadline = disconnected_at + grace;
        loop {
            if Instant::now() >= reconnect_deadline {
                connection.active.store(false, Ordering::Release);
                connection.fail_controls();
                connection
                    .tunnel_senders
                    .lock()
                    .expect("remote tunnel connection lock")
                    .clear();
                connection.tunnels.close_all(None).await;
                return;
            }
            match tokio::time::timeout_at(
                reconnect_deadline,
                connect_websocket(&connection.credentials),
            )
            .await
            {
                Ok(Ok(reconnected)) => {
                    socket = reconnected;
                    break;
                }
                Ok(Err(_)) => {
                    let retry_at =
                        (Instant::now() + Duration::from_secs(1)).min(reconnect_deadline);
                    tokio::time::sleep_until(retry_at).await;
                }
                Err(_) => {
                    connection.active.store(false, Ordering::Release);
                    connection.fail_controls();
                    connection
                        .tunnel_senders
                        .lock()
                        .expect("remote tunnel connection lock")
                        .clear();
                    connection.tunnels.close_all(None).await;
                    return;
                }
            }
        }
    }
}

fn handle_text(connection: &RemoteConnection, text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind == "session_moved" {
        connection.active.store(false, Ordering::Release);
        connection.fail_controls();
        return true;
    }
    if kind == "tunnel_closed" || kind == "tunnel_error" {
        if let Some(connection_id) = value.get("connectionId").and_then(Value::as_str) {
            connection.remove_tunnel_sender(connection_id);
        }
    }
    if kind == "channel" || kind == "channel_gap" {
        let Some(channel_id) = value.get("channelId").and_then(Value::as_u64) else {
            return false;
        };
        let message = if kind == "channel" {
            value.get("message").cloned().unwrap_or(Value::Null)
        } else {
            json!({"type":"channel_gap","dropped":value.get("dropped").cloned().unwrap_or(Value::from(0))})
        };
        if let Some(channel) = connection
            .channels
            .lock()
            .expect("remote channel registry lock")
            .get(&channel_id)
        {
            let _ = channel.send(message);
        }
        return false;
    }
    if let Some(key) = control_key(&value) {
        if let Some(sender) = connection
            .controls
            .lock()
            .expect("remote control registry lock")
            .remove(&key)
        {
            let _ = sender.send(value);
        }
    }
    false
}

async fn handle_binary(connection: &RemoteConnection, frame: &[u8]) {
    let Some((connection_id, payload)) = decode_tunnel_frame(frame) else {
        return;
    };
    let sender = connection
        .tunnel_senders
        .lock()
        .expect("remote tunnel connection lock")
        .get(connection_id)
        .cloned();
    if let Some(sender) = sender {
        let _ = sender.send(payload.to_vec()).await;
    }
}

fn control_key(value: &Value) -> Option<String> {
    let kind = value.get("type")?.as_str()?;
    let forward_id = value
        .get("forwardId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let connection_id = value
        .get("connectionId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "tunnel_registered" | "tunnel_unregistered" => Some(format!("{kind}:{forward_id}")),
        "tunnel_opened" | "tunnel_closed" => Some(format!("{kind}:{connection_id}")),
        "tunnel_error" if !connection_id.is_empty() => {
            Some(format!("tunnel_opened:{connection_id}"))
        }
        "tunnel_error" => Some(format!("tunnel_registered:{forward_id}")),
        _ => None,
    }
}

fn decode_tunnel_frame(frame: &[u8]) -> Option<(&str, &[u8])> {
    const ID_BYTES: usize = 36;
    const HEADER_BYTES: usize = 2 + ID_BYTES;
    const PAYLOAD_BYTES: usize = 16 * 1024;
    if frame.len() < 2 || usize::from(u16::from_be_bytes([frame[0], frame[1]])) != ID_BYTES {
        return None;
    }
    let payload = frame.get(HEADER_BYTES..)?;
    if payload.len() > PAYLOAD_BYTES {
        return None;
    }
    let id = std::str::from_utf8(frame.get(2..HEADER_BYTES)?).ok()?;
    is_canonical_uuid(id).then_some((id, payload))
}

fn is_canonical_uuid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
}

fn normalize_server_url(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    let url = Url::parse(value).map_err(|error| {
        AppError::new(
            "server URL must be a valid HTTP or HTTPS URL",
            AppErrorKind::Validation,
        )
        .with_source(error)
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(validation_error(
            "server URL must use HTTP or HTTPS and cannot include credentials, query, or fragment",
        ));
    }
    if url.scheme() == "http" {
        let host = url.host_str().expect("host validated above");
        let is_loopback = host == "localhost"
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if !is_loopback {
            return Err(validation_error("non-loopback server URLs require HTTPS"));
        }
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn api_url(server_url: &str, path: &str) -> Result<Url, AppError> {
    Url::parse(&format!(
        "{}/api/v1/{path}",
        server_url.trim_end_matches('/')
    ))
    .map_err(|error| {
        AppError::new("server URL is invalid", AppErrorKind::Validation).with_source(error)
    })
}

#[cfg(test)]
fn channel_id_from_marker(value: &str) -> Option<u64> {
    value
        .strip_prefix(CHANNEL_PREFIX)?
        .parse::<u64>()
        .ok()
        .filter(|id| *id < (1_u64 << 53))
}

async fn response_value(response: reqwest::Response) -> Result<Value, AppError> {
    let status = response.status();
    let value = response.json::<Value>().await.map_err(network_error)?;
    if status.is_success() {
        return Ok(value);
    }
    let error = serde_json::from_value::<AppError>(value).unwrap_or_else(|_| {
        AppError::new(
            "remote server returned an invalid error response",
            AppErrorKind::Transport,
        )
    });
    Err(error)
}

async fn response_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, AppError> {
    let status = response.status();
    if !status.is_success() {
        let error = response.json::<AppError>().await.unwrap_or_else(|_| {
            AppError::new(
                "remote server rejected the connection request",
                AppErrorKind::Transport,
            )
        });
        return Err(error);
    }
    response.json::<T>().await.map_err(network_error)
}

fn install_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn network_error(error: impl std::error::Error + Send + Sync + 'static) -> AppError {
    AppError::new(
        "remote server network request failed",
        AppErrorKind::Network,
    )
    .with_source(error)
}

fn validation_error(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Validation)
}

fn session_error(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Session)
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
