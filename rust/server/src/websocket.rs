use crate::{
    ApiError, AppState, AuthenticationRevoked, access::AccessIdentity, access_subject,
    authentication_ended, sessions::Owner, tunnel::TunnelReply,
};
use axum::{
    Extension,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::{FutureExt, SinkExt, StreamExt};
use kubecove_lib::models::{AppError, AppErrorKind, MAX_TUNNEL_FRAME_BYTES, PortForwardRequest};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    resume_token: String,
}

pub async fn upgrade(
    State(state): State<Arc<AppState>>,
    Query(connection): Query<Connection>,
    identity: Option<Extension<AccessIdentity>>,
    revoked: Option<Extension<AuthenticationRevoked>>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let owner = state
        .sessions
        .owner_as(&connection.resume_token, access_subject(identity.as_ref()))
        .await?;
    Ok(ws
        .max_message_size(MAX_TUNNEL_FRAME_BYTES)
        .on_upgrade(move |socket| run(socket, state, owner, identity, revoked)))
}

async fn run(
    socket: WebSocket,
    state: Arc<AppState>,
    owner: Arc<Owner>,
    identity: Option<Extension<AccessIdentity>>,
    revoked: Option<Extension<AuthenticationRevoked>>,
) {
    let generation = state.sessions.connect(&owner).await;
    let (mut outgoing, mut incoming) = socket.split();
    // Same pending budget as one tunnel connection: RELAY_QUEUE_MESSAGES chunks of 16 KiB.
    let (responses, mut pending_responses) = mpsc::channel(crate::tunnel::RELAY_QUEUE_MESSAGES);
    let moved = {
        let changed = owner.socket_changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if owner.socket.lock().await.generation() != generation {
            return;
        }
        let writer = async {
            let mut channel_message = None;
            loop {
                let ready = owner.channels.ready.notified();
                tokio::pin!(ready);
                ready.as_mut().enable();
                if channel_message.is_none() {
                    channel_message = owner.channels.pop();
                }
                let message = tokio::select! {
                    message = pending_responses.recv() => match message { Some(message) => message, None => return },
                    () = std::future::ready(()), if channel_message.is_some() => {
                        Message::Text(channel_message.take().expect("pending channel frame"))
                    },
                    () = ready => continue,
                };
                if outgoing.send(message).await.is_err() {
                    return;
                }
            }
        };
        let reader = async {
            while let Some(Ok(frame)) = incoming.next().await {
                let response = match frame {
                    Message::Ping(bytes) => Some(Message::Pong(bytes)),
                    Message::Pong(_) => None,
                    Message::Text(text) => {
                        let message = Box::pin(owner.backend.scoped(control(
                            &owner,
                            &responses,
                            text.as_str(),
                        )))
                        .await;
                        Some(message)
                    }
                    Message::Binary(frame) => match owner.tunnels.send_binary(&frame).await {
                        Ok(Some(connection_id)) => Some(
                            TunnelReply::Closed {
                                connection_id: &connection_id,
                            }
                            .to_message(),
                        ),
                        Ok(None) => None,
                        Err(_) => return,
                    },
                    Message::Close(_) => return,
                };
                if let Some(response) = response
                    && responses.send(response).await.is_err()
                {
                    return;
                }
            }
        };
        // Reading and writing must progress independently to avoid duplex relay deadlocks.
        tokio::select! {
            biased;
            () = owner.ended.cancelled() => true,
            () = authentication_ended(identity.as_ref(), revoked.as_ref()) => false,
            () = changed => false,
            () = writer => false,
            () = reader => false,
        }
    };
    if moved {
        // A stalled peer must not prevent revocation. Send the notice when the sink is ready.
        let _ = outgoing
            .send(Message::Text("{\"type\":\"session_moved\"}".into()))
            .now_or_never();
    }
    let _ = outgoing.close().now_or_never();
    {
        let socket = owner.socket.lock().await;
        if socket.generation() == generation {
            owner.tunnels.close_connections();
        }
    }
    state.sessions.disconnected(owner, generation).await;
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum TunnelControl {
    #[serde(rename = "tunnel_register")]
    Register {
        forward_id: String,
        request: PortForwardRequest,
    },
    #[serde(rename = "tunnel_unregister")]
    Unregister { forward_id: String },
    #[serde(rename = "tunnel_open")]
    Open {
        forward_id: String,
        connection_id: String,
    },
    #[serde(rename = "tunnel_close")]
    Close { connection_id: String },
    #[serde(rename = "tunnel_eof")]
    Eof { connection_id: String },
}

async fn control(owner: &Arc<Owner>, responses: &mpsc::Sender<Message>, text: &str) -> Message {
    let Ok(control) = serde_json::from_str::<TunnelControl>(text) else {
        let error = AppError::new("invalid tunnel control frame", AppErrorKind::Validation);
        return tunnel_error(None, None, &error);
    };
    let _gate = owner.request_gate.read().await;
    if owner.ended.is_cancelled() {
        return tunnel_error(
            None,
            None,
            &AppError::new("session ended", AppErrorKind::Session),
        );
    }
    match control {
        TunnelControl::Register {
            forward_id,
            request,
        } => match owner.tunnels.register(forward_id.clone(), request).await {
            Ok(target) => TunnelReply::Registered {
                forward_id: &forward_id,
                resolved_pod_name: &target.pod_name,
                resolved_pod_port: target.pod_port,
                kubeconfig_source_key: target.kubeconfig_source_key.as_deref(),
                kubeconfig_source_label: target.kubeconfig_source_label.as_deref(),
            }
            .to_message(),
            Err(error) => tunnel_error(Some(&forward_id), None, &error),
        },
        TunnelControl::Unregister { forward_id } => {
            owner.tunnels.unregister(&forward_id);
            TunnelReply::Unregistered {
                forward_id: &forward_id,
            }
            .to_message()
        }
        TunnelControl::Open {
            forward_id,
            connection_id,
        } => {
            match owner
                .tunnels
                .open(&forward_id, connection_id.clone(), responses.clone())
                .await
            {
                Ok(()) => TunnelReply::Opened {
                    forward_id: &forward_id,
                    connection_id: &connection_id,
                }
                .to_message(),
                Err(error) => tunnel_error(Some(&forward_id), Some(&connection_id), &error),
            }
        }
        TunnelControl::Close { connection_id } => {
            owner.tunnels.close_connection(&connection_id);
            TunnelReply::Closed {
                connection_id: &connection_id,
            }
            .to_message()
        }
        TunnelControl::Eof { connection_id } => {
            let reply = if owner.tunnels.finish_upload(&connection_id) {
                TunnelReply::EofAck {
                    connection_id: &connection_id,
                }
            } else {
                TunnelReply::Closed {
                    connection_id: &connection_id,
                }
            };
            reply.to_message()
        }
    }
}

fn tunnel_error(
    forward_id: Option<&str>,
    connection_id: Option<&str>,
    error: &AppError,
) -> Message {
    TunnelReply::Error {
        forward_id,
        connection_id,
        error,
    }
    .to_message()
}
