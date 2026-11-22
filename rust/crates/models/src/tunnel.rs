//! Binary WebSocket frames that carry tunnelled TCP bytes between the desktop and
//! `kubecove-server`: a big-endian `u16` connection id length, the connection id, then
//! the payload.

/// Connection ids are canonical lowercase hyphenated UUIDs.
pub const TUNNEL_CONNECTION_ID_BYTES: usize = 36;
pub const MAX_TUNNEL_PAYLOAD_BYTES: usize = 16 * 1024;
const HEADER_BYTES: usize = 2 + TUNNEL_CONNECTION_ID_BYTES;
pub const MAX_TUNNEL_FRAME_BYTES: usize = HEADER_BYTES + MAX_TUNNEL_PAYLOAD_BYTES;

const _: () = assert!(TUNNEL_CONNECTION_ID_BYTES <= u16::MAX as usize);
const ID_LENGTH_PREFIX: [u8; 2] = (TUNNEL_CONNECTION_ID_BYTES as u16).to_be_bytes();

pub fn is_tunnel_connection_id(value: &str) -> bool {
    value.len() == TUNNEL_CONNECTION_ID_BYTES
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

/// Frames one payload chunk. The caller has checked the id with [`is_tunnel_connection_id`]
/// and kept the payload within [`MAX_TUNNEL_PAYLOAD_BYTES`].
pub fn encode_tunnel_frame(connection_id: &str, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_BYTES + payload.len());
    frame.extend_from_slice(&ID_LENGTH_PREFIX);
    frame.extend_from_slice(connection_id.as_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Splits a frame into its connection id and payload without copying. Truncated frames,
/// unknown id lengths, invalid ids and oversized payloads are rejected.
pub fn decode_tunnel_frame(frame: &[u8]) -> Option<(&str, &[u8])> {
    if frame.get(..2)? != ID_LENGTH_PREFIX {
        return None;
    }
    let payload = frame.get(HEADER_BYTES..)?;
    if payload.len() > MAX_TUNNEL_PAYLOAD_BYTES {
        return None;
    }
    let connection_id = std::str::from_utf8(&frame[2..HEADER_BYTES]).ok()?;
    is_tunnel_connection_id(connection_id).then_some((connection_id, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "123e4567-e89b-12d3-a456-426614174000";

    #[test]
    fn accepts_only_canonical_lowercase_uuid_ids() {
        assert!(is_tunnel_connection_id(ID));
        for value in [
            "",
            "123e4567e89b12d3a456426614174000",
            "123E4567-E89B-12D3-A456-426614174000",
            "123e4567-e89b-12d3-a456-42661417400g",
            "123e4567-e89b-12d3-a456-426614174000x",
        ] {
            assert!(!is_tunnel_connection_id(value), "accepted {value}");
        }
    }

    #[test]
    fn decodes_what_it_encodes_and_preserves_every_byte() {
        let frame = encode_tunnel_frame(ID, &[0, 255, 4, 0, 13]);
        assert_eq!(&frame[..2], &[0, 36]);
        let (parsed_id, payload) = decode_tunnel_frame(&frame).expect("valid frame");
        assert_eq!(parsed_id, ID);
        assert_eq!(payload, [0, 255, 4, 0, 13]);
        assert_eq!(
            decode_tunnel_frame(&encode_tunnel_frame(ID, &[])),
            Some((ID, &[][..]))
        );
    }

    #[test]
    fn rejects_truncated_unknown_and_oversized_frames() {
        assert!(decode_tunnel_frame(&[]).is_none());
        assert!(decode_tunnel_frame(&[0]).is_none());
        assert!(decode_tunnel_frame(&[0, 0]).is_none());
        assert!(decode_tunnel_frame(&[0, 36]).is_none());

        let mut unknown_id = Vec::from(2_u16.to_be_bytes());
        unknown_id.extend_from_slice(b"xx");
        assert!(decode_tunnel_frame(&unknown_id).is_none());

        let invalid_id = encode_tunnel_frame(&ID.to_uppercase(), b"data");
        assert!(decode_tunnel_frame(&invalid_id).is_none());

        let largest = encode_tunnel_frame(ID, &vec![0; MAX_TUNNEL_PAYLOAD_BYTES]);
        assert_eq!(largest.len(), MAX_TUNNEL_FRAME_BYTES);
        assert!(decode_tunnel_frame(&largest).is_some());
        let oversized = encode_tunnel_frame(ID, &vec![0; MAX_TUNNEL_PAYLOAD_BYTES + 1]);
        assert!(decode_tunnel_frame(&oversized).is_none());
    }
}
