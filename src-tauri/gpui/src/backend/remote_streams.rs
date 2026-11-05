use super::*;
impl DesktopBackend {
    pub(super) fn stream(&self, command: &'static str, mut args: Value) -> Read<LogStream> {
        let remote = self.remote.clone();
        let id = self.channel.fetch_add(1, Ordering::Relaxed);
        let runtime = self
            .local
            .runtime
            .as_ref()
            .expect("runtime exists")
            .handle()
            .clone();
        self.local.read(async move {
            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            let channel = kubecove_lib::runtime::Channel::new(
                move |raw: Box<serde_json::value::RawValue>| {
                    let message =
                        serde_json::from_str(raw.get()).unwrap_or_else(|_| StreamMessage::Error {
                            stream_id: String::new(),
                            message: "Invalid remote stream message".into(),
                        });
                    sender
                        .send(message)
                        .map_err(|_| kubecove_lib::runtime::ChannelClosed)
                },
            );
            remote::remote_subscribe_channel(id, channel, State::new(&remote)).await?;
            // Create the guard before opening so cancellation also unsubscribes.
            let mut stop = StopRemoteStream {
                remote: remote.clone(),
                channel: id,
                stream: None,
                runtime,
            };
            args["channel"] = json!(format!("__KUBECOVE_CHANNEL__:{id}"));
            stop.stream = Some(invoke(remote, command, args).await?);
            Ok(LogStream::new(receiver, stop))
        })
    }
}
struct StopRemoteStream {
    remote: Arc<RemoteConnectionStore>,
    channel: u64,
    stream: Option<String>,
    runtime: tokio::runtime::Handle,
}
impl Drop for StopRemoteStream {
    fn drop(&mut self) {
        let (remote, channel, stream) = (self.remote.clone(), self.channel, self.stream.take());
        self.runtime.spawn(async move {
            if let Some(id) = stream {
                let _: Result<Value, _> =
                    invoke(remote.clone(), "stop_stream", json!({"streamId":id})).await;
            }
            let _ = remote::remote_unsubscribe_channel(channel, State::new(&remote)).await;
        });
    }
}
