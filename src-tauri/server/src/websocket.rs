use crate::{
    access::AccessIdentity, access_subject, authentication_ended, sessions::Owner, ApiError,
    AppState, AuthenticationRevoked,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
    Extension,
};
use futures_util::{FutureExt, SinkExt, StreamExt};
use kubecove_lib::models::{AppError, AppErrorKind, PortForwardRequest, MAX_TUNNEL_FRAME_BYTES};
use serde::Deserialize;
use serde_json::{json, Value};
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
        if owner.socket.lock().await.generation != generation {
            return;
        }
        let writer = async {
            let mut channel_message: Option<String> = None;
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
                        Message::Text(channel_message.take().expect("pending channel frame").into())
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
                        let message = owner
                            .backend
                            .scoped(control(&owner, &responses, text.as_str()))
                            .await;
                        Some(Message::Text(message.to_string().into()))
                    }
                    Message::Binary(frame) => match owner.tunnels.send_binary(&frame).await {
                        Ok(Some(connection_id)) => Some(Message::Text(
                            json!({"type":"tunnel_closed","connectionId":connection_id})
                                .to_string()
                                .into(),
                        )),
                        Ok(None) => None,
                        Err(_) => return,
                    },
                    Message::Close(_) => return,
                };
                if let Some(response) = response {
                    if responses.send(response).await.is_err() {
                        return;
                    }
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
        if socket.generation == generation {
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

async fn control(owner: &Arc<Owner>, responses: &mpsc::Sender<Message>, text: &str) -> Value {
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
            Ok(target) => json!({"type":"tunnel_registered","forwardId":forward_id,
                "resolvedPodName":target.pod_name,"resolvedPodPort":target.pod_port,
                "kubeconfigSourceKey":target.kubeconfig_source_key,
                "kubeconfigSourceLabel":target.kubeconfig_source_label}),
            Err(error) => tunnel_error(Some(&forward_id), None, &error),
        },
        TunnelControl::Unregister { forward_id } => {
            owner.tunnels.unregister(&forward_id);
            json!({"type":"tunnel_unregistered","forwardId":forward_id})
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
                Ok(()) => {
                    json!({"type":"tunnel_opened","forwardId":forward_id,"connectionId":connection_id})
                }
                Err(error) => tunnel_error(Some(&forward_id), Some(&connection_id), &error),
            }
        }
        TunnelControl::Close { connection_id } => {
            owner.tunnels.close_connection(&connection_id);
            json!({"type":"tunnel_closed","connectionId":connection_id})
        }
        TunnelControl::Eof { connection_id } => {
            let message = if owner.tunnels.finish_upload(&connection_id) {
                "tunnel_eof_ack"
            } else {
                "tunnel_closed"
            };
            json!({"type":message,"connectionId":connection_id})
        }
    }
}

fn tunnel_error(forward_id: Option<&str>, connection_id: Option<&str>, error: &AppError) -> Value {
    json!({"type":"tunnel_error","forwardId":forward_id,"connectionId":connection_id,"error":error})
}
