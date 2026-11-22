//! Observe the same serialized stream contract on either host transport.

use crate::{models::StreamMessage, runtime::Channel};
use serde_json::Value;

pub(crate) fn stream_channel(
    sink: impl Fn(Value) -> Result<(), ()> + Send + Sync + 'static,
) -> Channel<StreamMessage> {
    Channel::new(move |message| {
        sink(serde_json::to_value(message).expect("stream JSON"))
            .map_err(|()| crate::runtime::ChannelClosed)
    })
}
