//! Offline WLOC response patch core (clean-room protocol notes).
//!
//! Implements the AppleWLoc payload transformation described in
//! `docs/protocol/WLOC_PROTOCOL_NOTES.md`: a bounded protobuf field parser,
//! recursive Location replacement inside WifiDevice (field 2) and
//! CellResponse (field 22/24) sub-messages, byte-for-byte preservation of
//! untouched fields, and envelope re-wrapping (synthetic / marker shapes).
//!
//! The transformation is **fail-open**: any parse failure, invalid
//! coordinate, or oversize input leaves the response unchanged. A malformed
//! response is never replaced with a broken or fabricated one.

use std::fmt;

/// Maximum protobuf payload accepted for patching.
const MAX_PATCH_PAYLOAD_BYTES: usize = 512 * 1024;
/// Stable marker that precedes the payload length in a marker envelope.
pub const WLOC_MARKER: [u8; 6] = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00];
/// Default horizontal accuracy written only when Apple already supplies it.
const DEFAULT_HORIZONTAL_ACCURACY: u64 = 39;
/// CellResponse sub-message field numbers at the root.
const CELL_RESPONSE_FIELDS: [u32; 2] = [22, 24];

/// Errors from parsing or patching a WLOC payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WlocError {
    /// Payload exceeds the patching bound.
    Oversized,
    /// A varint runs past the buffer or exceeds 10 bytes.
    Truncated,
    /// A protobuf field uses a wire type the parser does not accept.
    UnsupportedWireType(u8),
    /// Field number 0 is invalid protobuf.
    InvalidFieldNumber,
    /// The envelope length field is inconsistent with the buffer.
    InvalidEnvelope,
    /// The requested coordinates are outside the valid ranges.
    InvalidCoordinates,
    /// The payload does not parse as a WLOC protobuf.
    NotWlocPayload,
}

impl fmt::Display for WlocError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized => formatter.write_str("WLOC payload exceeds the patch bound"),
            Self::Truncated => formatter.write_str("WLOC protobuf is truncated"),
            Self::UnsupportedWireType(wire) => {
                write!(formatter, "unsupported protobuf wire type {wire}")
            }
            Self::InvalidFieldNumber => formatter.write_str("protobuf field number 0"),
            Self::InvalidEnvelope => formatter.write_str("invalid WLOC envelope"),
            Self::InvalidCoordinates => formatter.write_str("invalid WLOC patch coordinates"),
            Self::NotWlocPayload => formatter.write_str("not a parseable WLOC payload"),
        }
    }
}

impl std::error::Error for WlocError {}

/// Coordinates and horizontal accuracy to write into an existing Location.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchTarget {
    pub latitude: f64,
    pub longitude: f64,
    pub horizontal_accuracy: u64,
}

impl PatchTarget {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            horizontal_accuracy: DEFAULT_HORIZONTAL_ACCURACY,
        }
    }

    /// Validate coordinates within the real-world ranges.
    pub fn validate(self) -> Result<Self, WlocError> {
        if !self.latitude.is_finite()
            || !self.longitude.is_finite()
            || !(-90.0..=90.0).contains(&self.latitude)
            || !(-180.0..=180.0).contains(&self.longitude)
        {
            return Err(WlocError::InvalidCoordinates);
        }
        Ok(self)
    }
}

/// Fixed-point coordinate encoding: `int64(coord * 1e8)`, truncated toward
/// zero, matching the reference implementation used on real devices.
pub fn coord_to_int(value: f64) -> i64 {
    (value * 1e8).trunc() as i64
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(((value as u8) & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Encode a varint field (wire type 0) with a signed 64-bit value.
pub fn encode_varint_field(field_number: u32, value: i64) -> Vec<u8> {
    let key = u64::from(field_number) << 3;
    let mut out = Vec::with_capacity(12);
    encode_varint(key, &mut out);
    // Negative values use the 64-bit two's complement unsigned encoding.
    encode_varint(value as u64, &mut out);
    out
}

/// Encode a length-delimited field (wire type 2).
pub fn encode_length_delimited_field(field_number: u32, payload: &[u8]) -> Vec<u8> {
    let key = (u64::from(field_number) << 3) | 2;
    let mut out = Vec::with_capacity(payload.len() + 10);
    encode_varint(key, &mut out);
    encode_varint(payload.len() as u64, &mut out);
    out.extend_from_slice(payload);
    out
}

/// A parsed protobuf field with its raw (key + value) and value spans.
#[derive(Debug, PartialEq)]
struct Field<'a> {
    field_number: u32,
    wire_type: u8,
    raw: &'a [u8],
    value_bytes: &'a [u8],
}

fn decode_varint(bytes: &[u8], start: usize) -> Result<(u64, usize), WlocError> {
    let mut value = 0_u64;
    for index in 0..10 {
        let position = start + index;
        let byte = *bytes.get(position).ok_or(WlocError::Truncated)?;
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok((value, position + 1));
        }
    }
    Err(WlocError::Truncated)
}

/// Parse a bounded protobuf payload into fields. Rejects field number 0,
/// unsupported wire types, overlong varints, and out-of-buffer fields.
fn parse_fields(bytes: &[u8]) -> Result<Vec<Field<'_>>, WlocError> {
    if bytes.len() > MAX_PATCH_PAYLOAD_BYTES {
        return Err(WlocError::Oversized);
    }
    let mut fields = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let key_start = offset;
        let (key, key_end) = decode_varint(bytes, offset)?;
        let field_number = (key >> 3) as u32;
        let wire_type = (key & 0x7) as u8;
        if field_number == 0 {
            return Err(WlocError::InvalidFieldNumber);
        }
        let (value_start, value_end) = match wire_type {
            0 => {
                let (_, end) = decode_varint(bytes, key_end)?;
                (key_end, end)
            }
            1 => (key_end, key_end.checked_add(8).ok_or(WlocError::Truncated)?),
            2 => {
                let (length, length_end) = decode_varint(bytes, key_end)?;
                let length = usize::try_from(length).map_err(|_| WlocError::Truncated)?;
                let end = length_end.checked_add(length).ok_or(WlocError::Truncated)?;
                (length_end, end)
            }
            5 => (key_end, key_end.checked_add(4).ok_or(WlocError::Truncated)?),
            other => return Err(WlocError::UnsupportedWireType(other)),
        };
        if value_end > bytes.len() {
            return Err(WlocError::Truncated);
        }
        fields.push(Field {
            field_number,
            wire_type,
            raw: &bytes[key_start..value_end],
            value_bytes: &bytes[value_start..value_end],
        });
        offset = value_end;
    }
    Ok(fields)
}

/// Replace only existing coordinate fields of a Location sub-message.
/// Every other field, including unrecognized fields, stays byte-for-byte.
fn patch_location(payload: &[u8], target: &PatchTarget) -> Result<Vec<u8>, WlocError> {
    let fields = parse_fields(payload)?;
    let mut parts = Vec::with_capacity(payload.len() + 64);
    for field in &fields {
        match (field.field_number, field.wire_type) {
            (1, 0) => parts.extend(encode_varint_field(1, coord_to_int(target.latitude))),
            (2, 0) => parts.extend(encode_varint_field(2, coord_to_int(target.longitude))),
            (3, 0) => parts.extend(encode_varint_field(3, target.horizontal_accuracy as i64)),
            _ => parts.extend_from_slice(field.raw),
        }
    }
    Ok(parts)
}

/// Patch an existing WifiDevice Location at field 2.
fn patch_wifi_device(payload: &[u8], target: &PatchTarget) -> Result<Vec<u8>, WlocError> {
    let fields = parse_fields(payload)?;
    let mut parts = Vec::with_capacity(payload.len() + 64);
    for field in &fields {
        if field.field_number == 2 && field.wire_type == 2 {
            parts.extend(encode_length_delimited_field(
                2,
                &patch_location(field.value_bytes, target)?,
            ));
        } else {
            parts.extend_from_slice(field.raw);
        }
    }
    Ok(parts)
}

/// Patch an existing CellResponse Location at field 5.
fn patch_cell_response(payload: &[u8], target: &PatchTarget) -> Result<Vec<u8>, WlocError> {
    let fields = parse_fields(payload)?;
    let mut parts = Vec::with_capacity(payload.len() + 64);
    for field in &fields {
        if field.field_number == 5 && field.wire_type == 2 {
            parts.extend(encode_length_delimited_field(
                5,
                &patch_location(field.value_bytes, target)?,
            ));
        } else {
            parts.extend_from_slice(field.raw);
        }
    }
    Ok(parts)
}

/// Patch the root AppleWLoc payload: a top-level Location (1), WifiDevice
/// (2) and CellResponse (22/24) locations are rewritten only when present;
/// every other root field is preserved.
pub fn patch_payload(payload: &[u8], target: &PatchTarget) -> Result<Vec<u8>, WlocError> {
    let fields = parse_fields(payload)?;
    let mut parts = Vec::with_capacity(payload.len() + 64);
    for field in &fields {
        match (field.field_number, field.wire_type) {
            (1, 2) => parts.extend(encode_length_delimited_field(
                1,
                &patch_location(field.value_bytes, target)?,
            )),
            (2, 2) => parts.extend(encode_length_delimited_field(
                2,
                &patch_wifi_device(field.value_bytes, target)?,
            )),
            (number, 2) if CELL_RESPONSE_FIELDS.contains(&number) => {
                parts.extend(encode_length_delimited_field(
                    number,
                    &patch_cell_response(field.value_bytes, target)?,
                ))
            }
            _ => parts.extend_from_slice(field.raw),
        }
    }
    Ok(parts)
}

fn read_u16_be(bytes: &[u8], offset: usize) -> Result<u16, WlocError> {
    let high = *bytes.get(offset).ok_or(WlocError::InvalidEnvelope)?;
    let low = *bytes.get(offset + 1).ok_or(WlocError::InvalidEnvelope)?;
    Ok((u16::from(high) << 8) | u16::from(low))
}

/// Result of extracting a payload from a recognized envelope shape.
enum Envelope<'a> {
    /// 8-byte prefix, u16 BE length, payload. Prefix and suffix preserved.
    Synthetic {
        prefix: &'a [u8],
        payload: &'a [u8],
        suffix: &'a [u8],
    },
    /// `00 00 00 <kind> 00 00` + u16 BE length + payload. Bytes before the
    /// marker preserved. `kind` echoes the request type (1 = BSSID query,
    /// 3 = coarse coordinate query).
    Marker {
        pre: &'a [u8],
        payload: &'a [u8],
        suffix: &'a [u8],
        kind: u8,
    },
    /// The real Apple gs-loc wifi response framing (confirmed against the live
    /// server): a 10-byte opaque header where `[0:2]` is `0x0001`, `[2:6]` an
    /// opaque marker, and `[6:10]` a big-endian uint32 block length, followed
    /// by the BlockBSSIDApple protobuf. The block length must be recomputed
    /// after a rewrite because the coordinate varints change size.
    Wloc10 {
        header: &'a [u8],
        payload: &'a [u8],
        suffix: &'a [u8],
    },
}

impl<'a> Envelope<'a> {
    /// The protobuf payload carried by this envelope.
    fn payload(&self) -> &'a [u8] {
        match self {
            Envelope::Synthetic { payload, .. }
            | Envelope::Marker { payload, .. }
            | Envelope::Wloc10 { payload, .. } => payload,
        }
    }
}

fn extract_envelope(response: &[u8]) -> Option<Envelope<'_>> {
    // Real gs-loc wifi response: 10-byte header, [6:10] = u32 BE block length.
    if response.len() >= 10 && response[0] == 0 && response[1] == 1 {
        let block_len =
            u32::from_be_bytes([response[6], response[7], response[8], response[9]]) as usize;
        if block_len > 0 && 10 + block_len <= response.len() {
            let payload = &response[10..10 + block_len];
            if parse_fields(payload).is_ok() {
                return Some(Envelope::Wloc10 {
                    header: &response[..10],
                    payload,
                    suffix: &response[10 + block_len..],
                });
            }
        }
    }
    // Synthetic shape: 8-byte prefix (bytes 6/7 zero), u16 BE length at 8.
    if response.len() >= 10 && response[6] == 0 && response[7] == 0 {
        if let Ok(length) = read_u16_be(response, 8) {
            let length = usize::from(length);
            if length > 0 && 10 + length <= response.len() {
                let payload = &response[10..10 + length];
                if parse_fields(payload).is_ok() {
                    return Some(Envelope::Synthetic {
                        prefix: &response[..8],
                        payload,
                        suffix: &response[10 + length..],
                    });
                }
            }
        }
    }
    // Marker fallback: search for the `00 00 00 <kind> 00 00` length prefix
    // anywhere. The kind byte echoes the request type (1 = BSSID query,
    // 3 = coarse coordinate query); iOS wraps every request in it.
    let mut index = 0;
    while index + 10 <= response.len() {
        if response[index] == 0
            && response[index + 1] == 0
            && response[index + 2] == 0
            && response[index + 4] == 0
            && response[index + 5] == 0
        {
            let kind = response[index + 3];
            let length_offset = index + 6;
            if let Ok(length) = read_u16_be(response, length_offset) {
                let length = usize::from(length);
                let payload_offset = length_offset + 2;
                if length > 0 && payload_offset + length <= response.len() {
                    let payload = &response[payload_offset..payload_offset + length];
                    if parse_fields(payload).is_ok() {
                        return Some(Envelope::Marker {
                            pre: &response[..index],
                            payload,
                            suffix: &response[payload_offset + length..],
                            kind,
                        });
                    }
                }
            }
        }
        index += 1;
    }
    None
}

/// Patch a WLOC response in place, preserving the envelope shape and all
/// bytes outside the payload. **Fail-open**: any error returns the original
/// response unchanged.
pub fn patch_wloc_response(response: &[u8], target: &PatchTarget) -> Vec<u8> {
    match try_patch_wloc_response(response, target) {
        Ok(patched) => patched,
        Err(_) => response.to_vec(),
    }
}

fn try_patch_wloc_response(response: &[u8], target: &PatchTarget) -> Result<Vec<u8>, WlocError> {
    let target = target.validate()?;
    if response.len() > MAX_PATCH_PAYLOAD_BYTES + 32 {
        return Err(WlocError::Oversized);
    }
    let envelope = extract_envelope(response).ok_or(WlocError::NotWlocPayload)?;
    let patched_payload = patch_payload(envelope.payload(), &target)?;
    wrap_envelope(envelope, &patched_payload)
}

/// Re-wrap a rewritten payload in the original envelope shape, recomputing
/// the length field so the byte layout stays valid for locationd.
fn wrap_envelope(envelope: Envelope<'_>, payload: &[u8]) -> Result<Vec<u8>, WlocError> {
    match envelope {
        Envelope::Synthetic { prefix, suffix, .. } => {
            let length = u16::try_from(payload.len()).map_err(|_| WlocError::Oversized)?;
            let mut out = Vec::with_capacity(prefix.len() + 2 + payload.len() + suffix.len());
            out.extend_from_slice(prefix);
            out.extend(u16_be(length));
            out.extend_from_slice(payload);
            out.extend_from_slice(suffix);
            Ok(out)
        }
        Envelope::Marker {
            pre, suffix, kind, ..
        } => {
            let length = u16::try_from(payload.len()).map_err(|_| WlocError::Oversized)?;
            let mut out = Vec::with_capacity(pre.len() + 8 + payload.len() + suffix.len());
            out.extend_from_slice(pre);
            out.extend([0x00, 0x00, 0x00, kind, 0x00, 0x00]);
            out.extend(u16_be(length));
            out.extend_from_slice(payload);
            out.extend_from_slice(suffix);
            Ok(out)
        }
        Envelope::Wloc10 { header, suffix, .. } => {
            let length = u32::try_from(payload.len()).map_err(|_| WlocError::Oversized)?;
            let mut out_header = [0_u8; 10];
            out_header.copy_from_slice(header);
            out_header[6..10].copy_from_slice(&length.to_be_bytes());
            let mut out = Vec::with_capacity(10 + payload.len() + suffix.len());
            out.extend_from_slice(&out_header);
            out.extend_from_slice(payload);
            out.extend_from_slice(suffix);
            Ok(out)
        }
    }
}

fn u16_be(value: u16) -> [u8; 2] {
    [(value >> 8) as u8, value as u8]
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn coord_to_int_matches_fixed_point_scaling() {
        // IEEE-754 truncation matches the reference `Math.trunc(coord * 1e8)`:
        // 37.3349 * 1e8 = 3733489999.9999995 -> 3733489999 (not ...49000).
        assert_eq!(coord_to_int(37.3349), 3_733_489_999);
        assert_eq!(coord_to_int(-122.00902), -12_200_902_000);
        assert_eq!(coord_to_int(0.0), 0);
        // Truncation toward zero (not rounding).
        assert_eq!(coord_to_int(1.000000004), 100_000_000);
    }

    #[test]
    fn field_encoders_round_trip_through_the_parser() {
        let payload = [
            encode_varint_field(1, -12_200_902_000),
            encode_length_delimited_field(2, b"hello"),
        ]
        .concat();
        let fields = parse_fields(&payload).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].field_number, 1);
        assert_eq!(fields[0].wire_type, 0);
        assert_eq!(fields[1].field_number, 2);
        assert_eq!(fields[1].wire_type, 2);
        assert_eq!(fields[1].value_bytes, b"hello");
    }

    #[test]
    fn unknown_wire_type_is_rejected() {
        let payload = [((3_u64 << 3) | 3) as u8]; // wire type 3 (group start)
        assert_eq!(
            parse_fields(&payload),
            Err(WlocError::UnsupportedWireType(3))
        );
    }
}
