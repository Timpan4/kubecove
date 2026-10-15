//! Observe the same serialized stream contract on either host transport.

use crate::{models::StreamMessage, runtime::Channel};
use serde_json::Value;

pub(crate) fn stream_channel(
    sink: impl Fn(Value) -> Result<(), ()> + Send + Sync + 'static,
) -> Channel<StreamMessage> {
    #[cfg(feature = "desktop")]
    {
        Channel::new(move |body| {
            let tauri::ipc::InvokeResponseBody::Json(body) = body else {
                panic!("expected JSON")
            };
            sink(serde_json::from_str(&body).expect("stream JSON"))
                .map_err(|()| std::io::Error::other("test consumer closed").into())
        })
    }
    #[cfg(not(feature = "desktop"))]
    {
        Channel::new(move |message| {
            sink(serde_json::to_value(message).expect("stream JSON"))
                .map_err(|()| crate::runtime::ChannelClosed)
        })
    }
}
