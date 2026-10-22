use super::client::{RemoteConnection, MAX_TUNNEL_PAYLOAD_BYTES};
use kubecove_models::{
    AppError, AppErrorKind, CommandArgs, PortForwardRequest, PortForwardSessionSummary,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    future::{ready, Future, Ready},
    net::{Ipv4Addr, SocketAddrV4},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::{JoinHandle, JoinSet},
};
use uuid::Uuid;

const FORWARD_QUEUE_MESSAGES: usize = 16;

#[derive(Default)]
pub struct RemoteTunnelRegistry {
    state: Mutex<HashMap<String, RemoteForward>>,
}

struct RemoteForward {
    summary: PortForwardSessionSummary,
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TunnelRegistration {
    resolved_pod_name: String,
    resolved_pod_port: u16,
}

impl RemoteTunnelRegistry {
    pub async fn start_pod_port_forward(
        &self,
        connection: &Arc<RemoteConnection>,
        mut request: PortForwardRequest,
    ) -> Result<PortForwardSessionSummary, AppError> {
        validate_request(&request)?;
        let listener = TcpListener::bind(SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            request.local_port.unwrap_or(0) as u16,
        ))
        .await
        .map_err(|error| {
            AppError::new(
                format!("local port unavailable: {error}"),
                AppErrorKind::LiveSessionTargetUnavailable,
            )
        })?;
        let local_port = listener.local_addr().map_err(io_error)?.port();
        request.local_port = Some(i64::from(local_port));
        let forward_id = Uuid::new_v4().to_string();
        let registration = connection
            .send_control(
                json!({"type":"tunnel_register","forwardId":forward_id,"request":request}),
                format!("tunnel_registered:{forward_id}"),
            )
            .await?;
        let registration: TunnelRegistration =
            serde_json::from_value(registration).map_err(|error| {
                AppError::new(
                    "remote server returned an invalid tunnel registration",
                    AppErrorKind::Session,
                )
                .with_source(error)
            })?;

        let target_kind = request
            .target_kind
            .as_deref()
            .unwrap_or(if request.pod_name.is_some() {
                "Pod"
            } else {
                "Service"
            })
            .to_string();
        let target_name = request
            .target_name
            .clone()
            .or_else(|| request.pod_name.clone())
            .unwrap_or_default();
        let started_at = chrono::Utc::now().to_rfc3339();
        let summary = PortForwardSessionSummary {
            id: forward_id.clone(),
            cluster_context: request.cluster_context.clone(),
            kubeconfig_env_var: request.kubeconfig_env_var.clone(),
            kubeconfig_source_key: None,
            kubeconfig_source_label: None,
            namespace: request.namespace.clone(),
            target_kind,
            target_name,
            pod_name: registration.resolved_pod_name.clone(),
            remote_port: u16::try_from(request.remote_port)
                .map_err(|_| validation_error("remote port must be between 1 and 65535"))?,
            resolved_pod_name: registration.resolved_pod_name,
            resolved_pod_port: registration.resolved_pod_port,
            local_port,
            local_address: "127.0.0.1".to_string(),
            local_url: format!("http://127.0.0.1:{local_port}"),
            status: "listening".to_string(),
            started_at,
            last_error: None,
        };

        let (stop, stopped) = oneshot::channel();
        let connection_task = Arc::clone(connection);
        let forward_task_id = forward_id.clone();
        let task = tokio::spawn(async move {
            run_forward(listener, connection_task, forward_task_id, stopped).await;
        });
        self.state
            .lock()
            .expect("remote tunnel registry lock")
            .insert(
                forward_id,
                RemoteForward {
                    summary: summary.clone(),
                    stop,
                    task,
                },
            );
        Ok(summary)
    }

    pub async fn stop_port_forward(
        &self,
        connection: &Arc<RemoteConnection>,
        forward_id: String,
    ) -> Result<bool, AppError> {
        let Some(forward) = self
            .state
            .lock()
            .expect("remote tunnel registry lock")
            .remove(&forward_id)
        else {
            return Ok(false);
        };
        let _ = forward.stop.send(());
        forward.task.abort();
        connection
            .send_control(
                json!({"type":"tunnel_unregister","forwardId":forward_id}),
                format!("tunnel_unregistered:{forward_id}"),
            )
            .await?;
        Ok(true)
    }

    pub fn list_port_forwards(
        &self,
        _connection: &Arc<RemoteConnection>,
    ) -> Ready<Result<Vec<PortForwardSessionSummary>, AppError>> {
        ready(Ok(self.list()))
    }

    fn list(&self) -> Vec<PortForwardSessionSummary> {
        self.state
            .lock()
            .expect("remote tunnel registry lock")
            .values()
            .map(|forward| forward.summary.clone())
            .collect()
    }

    pub async fn close_all(&self, connection: Option<&RemoteConnection>) {
        let forwards = self
            .state
            .lock()
            .expect("remote tunnel registry lock")
            .drain()
            .map(|(_, forward)| forward)
            .collect::<Vec<_>>();
        for forward in forwards {
            let _ = forward.stop.send(());
            forward.task.abort();
            if let Some(connection) = connection {
                let id = forward.summary.id;
                let _ = connection
                    .send_control_without_reply(json!({"type":"tunnel_unregister","forwardId":id}))
                    .await;
            }
        }
    }
}

// The desktop remote client handles each `remote_intercepted` command with the registry
// method of the same name instead of forwarding it to the server.
macro_rules! intercept {
    (
        $this:ident, $connection:ident, $command:ident, $payload:ident;
        remote_intercepted $name:ident($($key:ident: $ty:ty),*)
    ) => {
        if $command == stringify!($name) {
            #[allow(unused_variables)]
            let args = CommandArgs::new($payload)?;
            let result = $this
                .$name($connection, $(args.get::<$ty>(stringify!($key))?),*)
                .await?;
            return serde_json::to_value(result).map(Some).map_err(|error| {
                AppError::new(
                    "port-forward result could not be serialized",
                    AppErrorKind::Serialization,
                )
                .with_source(error)
            });
        }
    };
    ($this:ident, $connection:ident, $command:ident, $payload:ident; $other:ident $($ignored:tt)*) => {};
}

macro_rules! remote_intercept {
    ($(
        $transport:ident $mode:ident fn $name:ident($($key:ident: $ty:ty),* $(,)?)
            $(with $($host:ident $(<$state:ty>)?),+)?;
    )*) => {
        impl RemoteTunnelRegistry {
            /// Runs `command` on the laptop when it is remote-intercepted; `None` forwards it.
            pub async fn invoke(
                &self,
                connection: &Arc<RemoteConnection>,
                command: &str,
                payload: &Value,
            ) -> Result<Option<Value>, AppError> {
                $(intercept!(self, connection, command, payload; $transport $name($($key: $ty),*));)*
                Ok(None)
            }
        }
    };
}

kubecove_models::command_contract!(remote_intercept);

async fn run_forward(
    listener: TcpListener,
    connection: Arc<RemoteConnection>,
    forward_id: String,
    mut stopped: oneshot::Receiver<()>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = &mut stopped => break,
            _ = connections.join_next(), if !connections.is_empty() => {},
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let connection = Arc::clone(&connection);
                    let forward_id = forward_id.clone();
                    connections.spawn(Box::pin(run_connection(stream, connection, forward_id)));
                }
                Err(_) => break,
            }
        }
    }
    connections.abort_all();
}

async fn run_connection(
    mut stream: TcpStream,
    connection: Arc<RemoteConnection>,
    forward_id: String,
) -> Result<(), AppError> {
    let connection_id = Uuid::new_v4().to_string();
    let (incoming, mut incoming_rx) = mpsc::channel::<Vec<u8>>(FORWARD_QUEUE_MESSAGES);
    connection.add_tunnel_sender(connection_id.clone(), incoming)?;
    if let Err(error) = connection
        .send_control(
            json!({"type":"tunnel_open","forwardId":forward_id,"connectionId":connection_id}),
            format!("tunnel_opened:{connection_id}"),
        )
        .await
    {
        connection.remove_tunnel_sender(&connection_id);
        return Err(error);
    }
    let (mut reader, mut writer) = stream.split();
    let upload = async {
        let mut buffer = [0; MAX_TUNNEL_PAYLOAD_BYTES];
        loop {
            let read = reader.read(&mut buffer).await.map_err(io_error)?;
            if read == 0 {
                return connection
                    .send_control_without_reply(
                        json!({"type":"tunnel_eof","connectionId":connection_id}),
                    )
                    .await;
            }
            connection
                .send_binary(&connection_id, &buffer[..read])
                .await?;
        }
    };
    let result = Box::pin(relay_stream(&mut writer, &mut incoming_rx, upload)).await;
    connection.remove_tunnel_sender(&connection_id);
    let _ = connection
        .send_control(
            json!({"type":"tunnel_close","connectionId":connection_id}),
            format!("tunnel_closed:{connection_id}"),
        )
        .await;
    result
}

async fn relay_stream<W, U>(
    writer: &mut W,
    incoming_rx: &mut mpsc::Receiver<Vec<u8>>,
    upload: U,
) -> Result<(), AppError>
where
    W: AsyncWrite + Unpin,
    U: Future<Output = Result<(), AppError>>,
{
    let download = async {
        while let Some(payload) = incoming_rx.recv().await {
            writer.write_all(&payload).await.map_err(io_error)?;
        }
        Ok::<(), AppError>(())
    };
    tokio::pin!(upload);
    tokio::pin!(download);
    tokio::select! {
        result = &mut upload => match result {
            Ok(()) => download.await,
            Err(error) => Err(error),
        },
        result = &mut download => result,
    }
}

fn validate_request(request: &PortForwardRequest) -> Result<(), AppError> {
    if request.remote_port < 1 || request.remote_port > i64::from(u16::MAX) {
        return Err(validation_error("remote port must be between 1 and 65535"));
    }
    if let Some(port) = request.local_port {
        if port != 0 && !(1024..=i64::from(u16::MAX)).contains(&port) {
            return Err(validation_error(
                "local port must be 0 or between 1024 and 65535",
            ));
        }
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> AppError {
    AppError::new("remote tunnel I/O failed", AppErrorKind::Io).with_source(error)
}
fn validation_error(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    // Exercises the production Rust client against the headless smoke's isolated
    // HTTP/WebSocket Kubernetes fixture, without needing a webview or display.
    #[tokio::test]
    #[ignore = "requires the running headless smoke fixture"]
    async fn native_tunnel_delivers_bytes_and_closes_after_upload_eof() {
        let connection = super::super::client::connect(
            &std::env::var("KUBECOVE_REMOTE_TEST_URL").expect("fixture URL"),
            &std::env::var("KUBECOVE_REMOTE_TEST_TOKEN").expect("fixture token"),
            None,
        )
        .await
        .expect("connect native client");
        let mut request: PortForwardRequest = serde_json::from_value(json!({
            "clusterContext": "fixture", "namespace": "default",
            "podName": "forward-target", "remotePort": 8080,
            "kubeconfigEnvVar": "KUBECOVE_SMOKE_CONFIG",
        }))
        .expect("fixture request");
        let forward = connection
            .tunnels
            .start_pod_port_forward(&connection, request.clone())
            .await
            .expect("start laptop listener");
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, forward.local_port))
            .await
            .expect("connect laptop listener");
        let expected = (0..MAX_TUNNEL_PAYLOAD_BYTES * (FORWARD_QUEUE_MESSAGES + 1))
            .map(|index| index.to_le_bytes()[0])
            .collect::<Vec<_>>();
        let (mut reader, mut writer) = stream.split();
        let upload = writer.write_all(&expected);
        let download = async {
            let mut received = vec![0; expected.len()];
            reader
                .read_exact(&mut received)
                .await
                .expect("read all response bytes");
            assert_eq!(received, expected);
        };
        let (result, ()) = tokio::join!(upload, download);
        result.expect("write request bytes");
        writer.shutdown().await.expect("signal upload EOF");
        let mut trailing = Vec::new();
        reader
            .read_to_end(&mut trailing)
            .await
            .expect("server closes idle stream");
        assert_eq!(trailing.as_slice(), b"");
        assert!(connection.tunnel_senders.lock().unwrap().is_empty());
        // A pod-side close or error must close the laptop reader even while its
        // write half remains open. Echo-only fixtures cannot prove these paths.
        for (pod_name, expected) in [
            ("forward-close", b"peer closes first".as_slice()),
            ("forward-error", b"".as_slice()),
        ] {
            request.pod_name = Some(pod_name.to_string());
            let forward = connection
                .tunnels
                .start_pod_port_forward(&connection, request.clone())
                .await
                .expect("start peer termination listener");
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, forward.local_port))
                .await
                .expect("connect peer termination listener");
            stream
                .write_all(b"peer closes first")
                .await
                .expect("write peer request");
            let mut response = Vec::new();
            stream
                .read_to_end(&mut response)
                .await
                .expect("peer termination reaches laptop");
            assert_eq!(response, expected);
            assert!(connection.tunnel_senders.lock().unwrap().is_empty());
        }
        connection.close().await;
        assert!(connection.tunnels.list().is_empty());
    }

    #[tokio::test]
    async fn delivers_remote_response_after_local_write_half_closes() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let client = TcpStream::connect(address)
            .await
            .expect("connect test client");
        let (stream, _) = listener.accept().await.expect("accept test client");
        let (mut client_reader, mut client_writer) = client.into_split();
        let (mut reader, mut writer) = stream.into_split();
        let (incoming, mut incoming_rx) = mpsc::channel(1);
        let upload = async move {
            let mut request = Vec::new();
            let mut buffer = [0; 32];
            loop {
                let read = reader.read(&mut buffer).await.map_err(io_error)?;
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            assert_eq!(request, b"request");
            incoming
                .send(b"response after EOF".to_vec())
                .await
                .expect("send response");
            Ok::<(), AppError>(())
        };
        let relay =
            tokio::spawn(async move { relay_stream(&mut writer, &mut incoming_rx, upload).await });
        client_writer
            .write_all(b"request")
            .await
            .expect("write request");
        client_writer
            .shutdown()
            .await
            .expect("half-close client write side");
        let mut response = Vec::new();
        client_reader
            .read_to_end(&mut response)
            .await
            .expect("read response");
        relay.await.expect("relay task").expect("relay result");
        assert_eq!(response, b"response after EOF");
    }
}
