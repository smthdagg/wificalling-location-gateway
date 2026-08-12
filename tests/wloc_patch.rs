//! WLOC response patch core contract (clean-room protocol notes).
//!
//! Covers the Location sub-message replacement, recursive WifiDevice /
//! CellResponse patching, envelope re-wrapping, byte-for-byte preservation of
//! untouched fields, and the fail-open pass-through guarantee.

use wificalling_location_gateway::wloc::{
    coord_to_int, encode_length_delimited_field, encode_varint_field, patch_payload,
    patch_wloc_response, PatchTarget, WLOC_MARKER,
};

const LAT: f64 = 51.5074; // London
const LON: f64 = -0.1278;

fn target() -> PatchTarget {
    PatchTarget::new(LAT, LON)
}

/// Build a Location sub-message with the given coordinate varints plus an
/// unknown field (7) to verify byte-for-byte preservation.
fn location_payload(lat: i64, lon: i64) -> Vec<u8> {
    [
        encode_varint_field(1, lat),
        encode_varint_field(2, lon),
        encode_length_delimited_field(7, b"unknown-kept"),
    ]
    .concat()
}

fn wifi_device(location: &[u8]) -> Vec<u8> {
    [
        encode_length_delimited_field(1, b"bssid"),
        encode_length_delimited_field(2, location),
    ]
    .concat()
}

fn cell_response(location: &[u8]) -> Vec<u8> {
    [
        encode_length_delimited_field(1, b"cellid"),
        encode_length_delimited_field(5, location),
    ]
    .concat()
}

fn root_payload() -> Vec<u8> {
    let wifi = wifi_device(&location_payload(
        coord_to_int(37.7749),
        coord_to_int(-122.4194),
    ));
    let cell = cell_response(&location_payload(
        coord_to_int(40.7128),
        coord_to_int(-74.0060),
    ));
    [
        encode_varint_field(3, 999), // root drop field
        encode_length_delimited_field(2, &wifi),
        encode_length_delimited_field(22, &cell),
        encode_length_delimited_field(9, b"preserved-root"),
    ]
    .concat()
}

/// Parse a payload into (field_number, wire_type, raw_bytes) tuples.
fn fields_of(payload: &[u8]) -> Vec<(u32, u8, Vec<u8>)> {
    let mut fields = Vec::new();
    let mut offset = 0;
    while offset < payload.len() {
        let mut value = 0_u64;
        let mut key_end = offset;
        loop {
            let byte = payload[key_end];
            key_end += 1;
            value |= u64::from(byte & 0x7f) << (7 * (key_end - offset - 1));
            if byte & 0x80 == 0 {
                break;
            }
        }
        let field_number = (value >> 3) as u32;
        let wire_type = (value & 0x7) as u8;
        let value_start = match wire_type {
            2 => {
                let mut len = 0_u64;
                let mut len_end = key_end;
                loop {
                    let byte = payload[len_end];
                    len_end += 1;
                    len |= u64::from(byte & 0x7f) << (7 * (len_end - key_end - 1));
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                let end = len_end + len as usize;
                fields.push((field_number, wire_type, payload[offset..end].to_vec()));
                end
            }
            0 => {
                let mut end = key_end;
                loop {
                    let byte = payload[end];
                    end += 1;
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                fields.push((field_number, wire_type, payload[offset..end].to_vec()));
                end
            }
            _ => key_end,
        };
        offset = value_start;
    }
    fields
}

// --- coordinate scaling ---

#[test]
fn coordinates_use_fixed_point_int64_scaling() {
    assert_eq!(coord_to_int(51.5074), 5_150_740_000);
    assert_eq!(coord_to_int(-0.1278), -12_780_000);
}

#[test]
fn invalid_coordinates_are_rejected_fail_open() {
    // Latitude out of range and non-finite values must leave the response
    // unchanged.
    let response = synthetic_envelope(&root_payload());
    for (lat, lon) in [
        (95.0, 0.0),
        (f64::NAN, 0.0),
        (0.0, 200.0),
        (f64::INFINITY, 0.0),
    ] {
        let patched = patch_wloc_response(&response, &PatchTarget::new(lat, lon));
        assert_eq!(patched, response, "must pass through for ({lat}, {lon})");
    }
}

// --- Location sub-message ---

#[test]
fn location_unknown_fields_are_preserved_byte_for_byte() {
    let original = location_payload(coord_to_int(37.7749), coord_to_int(-122.4194));
    let patched = patch_payload(&original, &target()).unwrap();

    // Field 7 (unknown) must survive byte-for-byte.
    let kept = fields_of(&patched)
        .into_iter()
        .find(|(number, _, _)| *number == 7)
        .expect("unknown field must survive");
    assert_eq!(kept.2, encode_length_delimited_field(7, b"unknown-kept"));
}

// --- Recursive wifi / cell patching (through a root payload) ---

fn strip_length_prefix(field: &[u8]) -> Vec<u8> {
    // Skip key varint + length varint; the remaining bytes are the value.
    let mut offset = 0;
    loop {
        let byte = field[offset];
        offset += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    loop {
        let byte = field[offset];
        offset += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    field[offset..].to_vec()
}

fn location_of_field(field: &[u8]) -> (i64, i64) {
    // field = key + length + nested payload. Skip both varints.
    let value = strip_length_prefix(field);
    let nested = fields_of(&value);
    let lat = nested
        .iter()
        .find(|(number, wire, _)| *number == 1 && *wire == 0)
        .expect("latitude field must exist");
    let lon = nested
        .iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 0)
        .expect("longitude field must exist");
    (decode_varint_field(&lat.2), decode_varint_field(&lon.2))
}

fn decode_varint_field(field: &[u8]) -> i64 {
    // Skip the key varint, then read the value varint as signed 64-bit.
    let mut offset = 0;
    loop {
        let byte = field[offset];
        offset += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    let mut value = 0_u64;
    let mut shift = 0;
    loop {
        let byte = field[offset];
        offset += 1;
        value |= u64::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
    }
    value as i64
}

#[test]
fn wifi_device_location_is_replaced() {
    let root = encode_length_delimited_field(
        2,
        &wifi_device(&location_payload(
            coord_to_int(37.7749),
            coord_to_int(-122.4194),
        )),
    );
    let patched = patch_payload(&root, &target()).unwrap();
    let fields = fields_of(&patched);

    let wifi = fields
        .iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 2)
        .expect("wifi field must remain");
    let wifi_value = strip_length_prefix(&wifi.2);
    let nested = fields_of(&wifi_value);
    let location = nested
        .iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 2)
        .expect("wifi must contain location");
    let (lat, lon) = location_of_field(&location.2);
    assert_eq!(lat, coord_to_int(LAT));
    assert_eq!(lon, coord_to_int(LON));
    // The wifi bssid field survives.
    assert!(nested.iter().any(|(number, _, _)| *number == 1));
}

#[test]
fn cell_response_location_is_replaced() {
    let root = encode_length_delimited_field(
        22,
        &cell_response(&location_payload(
            coord_to_int(40.7128),
            coord_to_int(-74.0060),
        )),
    );
    let patched = patch_payload(&root, &target()).unwrap();
    let fields = fields_of(&patched);

    let cell = fields
        .iter()
        .find(|(number, wire, _)| *number == 22 && *wire == 2)
        .expect("cell field must remain");
    let cell_value = strip_length_prefix(&cell.2);
    let nested = fields_of(&cell_value);
    let location = nested
        .iter()
        .find(|(number, wire, _)| *number == 5 && *wire == 2)
        .expect("cell must contain location");
    let (lat, lon) = location_of_field(&location.2);
    assert_eq!(lat, coord_to_int(LAT));
    assert_eq!(lon, coord_to_int(LON));
}

#[test]
fn missing_wifi_location_is_appended() {
    let wifi = encode_length_delimited_field(1, b"bssid-only"); // no field 2
    let root = encode_length_delimited_field(2, &wifi);
    let patched = patch_payload(&root, &target()).unwrap();
    let fields = fields_of(&patched);

    let wifi = fields
        .iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 2)
        .expect("wifi field must remain");
    let wifi_value = strip_length_prefix(&wifi.2);
    let nested = fields_of(&wifi_value);
    let location = nested
        .iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 2)
        .expect("location must be appended to wifi");
    assert!(!location.2.is_empty());
}

// --- Root payload ---

#[test]
fn root_drop_fields_are_removed_and_others_preserved() {
    let patched = patch_payload(&root_payload(), &target()).unwrap();
    let fields = fields_of(&patched);

    assert!(
        !fields.iter().any(|(number, _, _)| *number == 3),
        "root drop field 3 must be removed"
    );
    assert!(
        fields.iter().any(|(number, _, _)| *number == 9),
        "non-drop root field 9 must be preserved"
    );
    assert!(
        fields.iter().any(|(number, _, _)| *number == 2),
        "wifi field must remain"
    );
    assert!(
        fields.iter().any(|(number, _, _)| *number == 22),
        "cell field must remain"
    );
}

// --- Envelope handling ---

/// Build the real gs-loc wifi response framing: a 10-byte opaque header
/// ([0:2] = 0x0001, [6:10] = u32 BE block length) plus the protobuf block.
fn wloc10_envelope(payload: &[u8]) -> Vec<u8> {
    let mut header = [0_u8; 10];
    header[0] = 0x00;
    header[1] = 0x01;
    header[2..6].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    header[6..10].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    let mut out = header.to_vec();
    out.extend_from_slice(payload);
    out
}

#[test]
fn wloc10_envelope_is_patched_and_block_length_recomputed() {
    // Use a wifi root payload that contains a Location, so the patch changes
    // the block length and the header's u32 BE field must be recomputed.
    let original = wloc10_envelope(&root_payload());
    let patched = patch_wloc_response(&original, &target());

    // Header [0:2] marker preserved; [6:10] is the new block length.
    assert_eq!(&patched[..2], &[0x00, 0x01]);
    let old_len = u32::from_be_bytes([original[6], original[7], original[8], original[9]]) as usize;
    let new_len = u32::from_be_bytes([patched[6], patched[7], patched[8], patched[9]]) as usize;
    assert!(new_len > 0);
    assert_eq!(
        patched.len(),
        10 + new_len,
        "body must be header + declared block"
    );
    let _ = old_len;
}

#[test]
fn wloc10_envelope_replaces_wifi_location_coordinates() {
    let original = wloc10_envelope(&root_payload());
    let patched = patch_wloc_response(&original, &target());
    let new_len = u32::from_be_bytes([patched[6], patched[7], patched[8], patched[9]]) as usize;
    let block = &patched[10..10 + new_len];

    // Block root: field 2 (wifi) -> nested field 2 (location) -> field 1 lat.
    let wifi = fields_of(block)
        .into_iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 2)
        .expect("wifi field must remain");
    let wifi_value = strip_length_prefix(&wifi.2);
    let location = fields_of(&wifi_value)
        .into_iter()
        .find(|(number, wire, _)| *number == 2 && *wire == 2)
        .expect("wifi must contain a location");
    let (lat, lon) = location_of_field(&location.2);
    assert_eq!(lat, coord_to_int(LAT));
    assert_eq!(lon, coord_to_int(LON));
}

fn synthetic_envelope(payload: &[u8]) -> Vec<u8> {
    let prefix = [0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00];
    let mut out = Vec::new();
    out.extend_from_slice(&prefix);
    out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(b"TRAILER");
    out
}

#[test]
fn synthetic_envelope_is_patched_and_rewrapped() {
    let response = synthetic_envelope(&root_payload());
    let patched = patch_wloc_response(&response, &target());

    // Prefix (8 bytes) and trailer preserved.
    assert_eq!(
        &patched[..8],
        &[0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]
    );
    assert!(patched.ends_with(b"TRAILER"));
    // New payload length is written.
    let length = u16::from_be_bytes([patched[8], patched[9]]) as usize;
    assert!(length > 0);
    let payload = &patched[10..10 + length];
    let fields = fields_of(payload);
    assert!(
        fields.iter().any(|(number, _, _)| *number == 2),
        "patched envelope must contain a wifi field"
    );
}

#[test]
fn marker_envelope_is_patched() {
    let mut response = b"JUNK-PREFIX-".to_vec();
    response.extend_from_slice(&WLOC_MARKER);
    response.extend_from_slice(&(root_payload().len() as u16).to_be_bytes());
    response.extend_from_slice(&root_payload());
    response.extend_from_slice(b"END");

    let patched = patch_wloc_response(&response, &target());
    assert!(patched.starts_with(b"JUNK-PREFIX-"));
    assert!(patched.ends_with(b"END"));
    // Marker still present, with a rewritten length.
    let marker_index = patched
        .windows(WLOC_MARKER.len())
        .position(|w| w == &WLOC_MARKER[..])
        .expect("marker must remain");
    let length =
        u16::from_be_bytes([patched[marker_index + 6], patched[marker_index + 7]]) as usize;
    assert!(length > 0);
    assert!(patched.ends_with(b"END"));
}

// --- Fail-open ---

#[test]
fn malformed_response_passes_through_unchanged() {
    for garbage in [
        b"".as_slice(),
        b"not-a-wloc-response".as_slice(),
        b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x02garbage".as_slice(), // length claims 2 but invalid protobuf
        vec![0xff; 128].as_slice(),
    ] {
        let patched = patch_wloc_response(garbage, &target());
        assert_eq!(
            patched,
            garbage.to_vec(),
            "malformed input must pass through unchanged"
        );
    }
}

#[test]
fn payload_with_no_location_fields_is_still_rewrapped() {
    let bare = encode_length_delimited_field(9, b"no-location");
    let response = synthetic_envelope(&bare);
    let patched = patch_wloc_response(&response, &target());
    // Non-drop root fields are preserved; envelope rewrapped.
    assert!(patched.starts_with(&[0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]));
    let length = u16::from_be_bytes([patched[8], patched[9]]) as usize;
    assert_eq!(&patched[10..10 + length], &bare);
}
