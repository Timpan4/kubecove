use super::client::{RemoteConnection, MAX_TUNNEL_PAYLOAD_BYTES};
use kubecove_live::commands::sessions::{target::validate_target, PortForwardRegistry};
use kubecove_models::{AppError, AppErrorKind, PortForwardRequest, PortForwardSessionSummary};
use serde::Deserialize;
use serde_json::json;
use std::{future::Future, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use uuid::Uuid;

const FORWARD_QUEUE_MESSAGES: usize = 16;

/// The desktop tunnel adapter of the Port Forward Lifecycle: forwards listen
/// on the laptop and relay each connection over the remote server's socket.
#[derive(Default)]
pub struct RemoteTunnelRegistry {
    forwards: PortForwardRegistry,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TunnelRegistration {
    resolved_pod_name: String,
    resolved_pod_port: u16,
}

impl RemoteTunnelRegistry {
    pub async fn start(
        &self,
        connection: Arc<RemoteConnection>,
        mut request: PortForwardRequest,
    ) -> Result<PortForwardSessionSummary, AppError> {
        let validated = validate_target(&request)?;
        let listener = self.forwards.bind(validated.local_port()).await?;
        request.local_port = Some(i64::from(listener.local_port()));
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
        let target = validated.resolved_by_server(
            registration.resolved_pod_name,
            registration.resolved_pod_port,
            None,
            None,
        );

        let relay_connection = Arc::clone(&connection);
        let relay_forward_id = forward_id.clone();
        let started = self
            .forwards
            .start(forward_id.clone(), &target, listener, move |stream| {
                Box::pin(run_connection(
                    stream,
                    Arc::clone(&relay_connection),
                    relay_forward_id.clone(),
                ))
            });
        if started.is_err() {
            let _ = connection
                .send_control_without_reply(unregister_control(&forward_id))
                .await;
        }
        started
    }

    pub async fn stop(
        &self,
        connection: &RemoteConnection,
        forward_id: &str,
    ) -> Result<bool, AppError> {
        if !self.forwards.stop(forward_id) {
            return Ok(false);
        }
        connection
            .send_control(
                unregister_control(forward_id),
                format!("tunnel_unregistered:{forward_id}"),
            )
            .await?;
        Ok(true)
    }

    pub fn list(&self) -> Vec<PortForwardSessionSummary> {
        self.forwards.list()
    }

    pub async fn close_all(&self, connection: Option<&RemoteConnection>) {
        let stopped = self.forwards.stop_all();
        if let Some(connection) = connection {
            for forward_id in stopped {
                let _ = connection
                    .send_control_without_reply(unregister_control(&forward_id))
                    .await;
            }
        }
    }

    pub async fn invoke(
        &self,
        connection: Arc<RemoteConnection>,
        command: &str,
        args: serde_json::Value,
    ) -> Result<Option<serde_json::Value>, AppError> {
        match command {
            "start_pod_port_forward" => {
                let request: PortForwardRequest = serde_json::from_value(
                    args.get("request").cloned().unwrap_or(args),
                )
                .map_err(|error| {
                    AppError::new("port-forward request is invalid", AppErrorKind::Validation)
                        .with_source(error)
                })?;
                let summary = self.start(connection, request).await?;
                serde_json::to_value(summary).map(Some).map_err(|error| {
                    AppError::new(
                        "port-forward summary could not be serialized",
                        AppErrorKind::Serialization,
                    )
                    .with_source(error)
                })
            }
            "stop_port_forward" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| validation_error("port-forward session id is required"))?;
                serde_json::to_value(self.stop(&connection, session_id).await?)
                    .map(Some)
                    .map_err(|error| {
                        AppError::new(
                            "port-forward result could not be serialized",
                            AppErrorKind::Serialization,
                        )
                        .with_source(error)
                    })
            }
            "list_port_forwards" => serde_json::to_value(self.list())
                .map(Some)
                .map_err(|error| {
                    AppError::new(
                        "port-forward list could not be serialized",
                        AppErrorKind::Serialization,
                    )
                    .with_source(error)
                }),
            _ => Ok(None),
        }
    }
}

fn unregister_control(forward_id: &str) -> serde_json::Value {
    json!({"type":"tunnel_unregister","forwardId":forward_id})
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

fn io_error(error: std::io::Error) -> AppError {
    AppError::new("remote tunnel I/O failed", AppErrorKind::Io).with_source(error)
}
fn validation_error(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
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
            .start(Arc::clone(&connection), request.clone())
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
                .start(Arc::clone(&connection), request.clone())
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
