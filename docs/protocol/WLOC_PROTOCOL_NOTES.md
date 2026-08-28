# WLOC response protocol notes (clean-room)

Status: derived from the project owner's existing implementation
(`smthdagg/ios-location-spoofer-plus`) and public protobuf facts. These notes
record the *protocol shape* — wire format, field numbers, and patch semantics
— as factual knowledge. The Rust implementation in `src/wloc/` is written
independently and does not copy the reference source. Per the Issue #1 license
ADR, the AGPL reference implementation (`mekos2772/ios-location-spoofer`) is
never ported; these notes only capture the protocol facts it demonstrates.

## 1. Envelope shapes

A `/clls/wloc` response body wraps the AppleWLoc protobuf in one of three
shapes. The patch core detects the shape, patches the payload, and rewrites
the same shape byte-for-byte around it.

1. **Synthetic**: an 8-byte prefix (commonly `00 01 00 00 00 01 00 00`),
   then `uint16 BE` payload length, then the protobuf payload. Detection
   requires bytes `[6] == 0` and `[7] == 0`, a non-zero length at offset 8,
   and that the payload parses as protobuf.
2. **Marker**: the stable marker `00 00 00 01 00 00` appears immediately
   before the `uint16 BE` payload length and payload. The synthetic prefix
   embeds this marker at offset 2, so synthetic is a specific marker
   alignment; the marker search is the general fallback.
3. **ARPC**: a request-style envelope — `uint16 BE` version, three
   Pascal strings (`uint16 BE` length + ASCII; locale, app identifier,
   OS version), `uint32 BE` functionId, `uint32 BE` payloadLength, then the
   payload. (Follow-up; responses normally use synthetic/marker.)

## 2. AppleWLoc protobuf payload

Standard protobuf wire format: a varint key `(field_number << 3) | wire_type`;
wire types 0 (varint), 1 (64-bit), 2 (length-delimited), 5 (32-bit).

Root fields of interest:

| field | wire | meaning |
|---|---|---|
| 2 | 2 | WifiDevice message |
| 22, 24 | 2 | CellResponse message |
| 3, 4, 33 | any | preserved unchanged |

`WifiDevice` contains the Location sub-message at **field 2 (wire 2)**;
`CellResponse` contains it at **field 5 (wire 2)**.

## 3. Location sub-message

Fields 1/2 are the coordinates, encoded as fixed-point varints:

```
value = int64(coord * 1e8)      # truncation toward zero, e.g. -122.00902 -> -12200902000
# negative values encoded as 64-bit two's complement unsigned varint
```

Only existing fields below are patched (all varints):

| field | meaning | default |
|---|---|---|
| 1 | latitude × 1e8 | target |
| 2 | longitude × 1e8 | target |
| 3 | horizontal accuracy (m) | 39 |

All other Location fields are preserved **byte-for-byte** (raw field bytes
copied unchanged, including unknown fields and non-minimal varints).

## 4. Patch algorithm (fail-open)

1. Parse the root payload into fields; reject unsupported wire types,
   field number 0, overlong varints, or fields exceeding the buffer.
2. For each root field:
   - field 2 (wire 2) → recursively patch `WifiDevice`, replacing its
     Location (field 2) when it exists.
   - field 22/24 (wire 2) → recursively patch `CellResponse`, replacing its
     Location (field 5) when it exists.
   - otherwise → preserve raw bytes.
3. In a Location, replace only existing fields 1/2/3 (latitude, longitude,
   horizontal accuracy); preserve every other field and every root field raw.
4. Rewrap in the detected envelope shape.

Any parse failure, invalid coordinate, or oversized input must leave the
response **unchanged** (fail-open pass-through); a malformed response is never
replaced with a broken or fabricated one.

## 5. Safety boundary

- Coordinates only enter the patch from the validated Geo resolver output
  (already bounded and country-code validated by the service).
- The patch core never installs a CA, never intercepts traffic, and never
  touches the transport; it is a pure payload transformation.
- Unknown fields stay unknown: the patch only rewrites the Location
  coordinate fields and preserves everything else byte-for-byte.
