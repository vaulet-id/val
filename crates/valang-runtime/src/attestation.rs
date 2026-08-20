//! The execution record, as something anybody's tooling can read.
//!
//! It used to be deterministic CBOR and a raw signature, verifiable by this
//! project's own library and nothing else — which is a strange thing for a
//! record whose whole purpose is to be handed to somebody who was not there.
//!
//! It is a **signed JWT** now: `EdDSA`, a public `vct`, the device's public key
//! in the header. Any JWS library verifies it, and what is left for
//! `valang-verify` is the part no standard covers — whether the code that ran is
//! the code somebody published, and whether the state went backwards.
//!
//! Deterministic CBOR has not gone anywhere. It is still what state roots and
//! code hashes are computed over, because those have to be canonical. What
//! changed is the envelope somebody else has to open.

use crate::{ExecutionRecord, Outcome};
use crate::merkle::hex;
use crate::value::Value;

/// The credential type of an execution record.
///
/// **Under the language's own name, not the first host's.** A second host issues
/// exactly this record, and a type identifier is not something that can be moved
/// later: every record already out there would name a type nobody uses. The
/// domain has to exist before the first record ships.
pub const VCT: &str = "https://val-lang.org/credential/execution-record/1";

fn b64(bytes: &[u8]) -> String {
    // base64url, unpadded — RFC 7515 §2. Fifteen lines rather than a dependency,
    // and the alphabet is the whole of the specification.
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        let take = chunk.len() + 1;
        for i in 0..take {
            out.push(A[(n >> (18 - 6 * i) & 0x3f) as usize] as char);
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    // A length that leaves one character over encodes nothing: six bits cannot
    // finish a byte, and a decoder that accepted it would be reading a string
    // nobody could have written.
    if s.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    // The bits left over are not part of any byte, and every encoder writes
    // them as zero. Accepting anything else made one signature reachable from
    // sixteen strings: the same record, verifying under sixteen tokens, and a
    // server that remembers what it has seen by the token it was handed would
    // have seen sixteen of them.
    if bits > 0 && acc & ((1 << bits) - 1) != 0 {
        return None;
    }
    Some(out)
}

/// A minimal JSON writer. The payload's shape is fixed and small, and pulling in
/// a serialiser to emit twenty known keys would be a dependency carried for
/// punctuation.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_value(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Str(s) => json_string(s),
        Value::Bytes(b) => json_string(&hex(&{
            let mut a = [0u8; 32];
            for (i, x) in b.iter().take(32).enumerate() {
                a[i] = *x;
            }
            a
        })),
        Value::List(items) => format!("[{}]", items.iter().map(json_value).collect::<Vec<_>>().join(",")),
        Value::Map(m) => format!(
            "{{{}}}",
            m.iter().map(|(k, v)| format!("{}:{}", json_string(k), json_value(v))).collect::<Vec<_>>().join(",")
        ),
        Value::Enum(e, member) => json_string(&format!("{e}.{member}")),
        Value::Credential { ty, claims, verified } => format!(
            "{{\"credential\":{},\"verified\":{},\"claims\":{}}}",
            json_string(ty),
            verified.as_ref().map_or("null".to_string(), |p| json_string(p)),
            json_value(&Value::Map(claims.clone()))
        ),
    }
}

fn outcome_str(o: &Outcome) -> String {
    match o {
        Outcome::Committed => "committed".into(),
        Outcome::Refused(w) => format!("refused: {w}"),
        Outcome::Failed(w) => format!("failed: {w}"),
        Outcome::Defect(w) => format!("defect: {w}"),
        Outcome::Declined(k) => format!("declined: {k}"),
    }
}

/// The claims, as JSON. Public so a host that prefers to sign with its own JWS
/// stack can take the payload and leave the envelope.
pub fn payload(r: &ExecutionRecord) -> String {
    let effects = r
        .effects_requested
        .iter()
        .map(|e| {
            format!(
                "{{\"capability\":{},\"reversible\":{},\"payload\":{}}}",
                json_string(&e.capability),
                e.reversible,
                json_value(&e.payload)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"vct\":{},\"iat\":{},\"app\":{},\"version\":{},\"action\":{},\
         \"code_hash\":{},\"input_hash\":{},\"previous_root\":{},\"next_root\":{},\
         \"policies\":[{}],\"capabilities\":[{}],\"effects\":[{}],\"executed\":{},\
         \"context\":{{\"time\":{},\"uuid\":{}}},\"outcome\":{}}}",
        json_string(VCT),
        r.context.time_now / 1000,
        json_string(&r.app),
        json_string(&r.version),
        json_string(&r.action),
        json_string(&hex(&r.code_hash)),
        json_string(&hex(&r.input_hash)),
        json_string(&hex(&r.previous_root)),
        json_string(&hex(&r.next_root)),
        r.policies.iter().map(|p| json_string(p)).collect::<Vec<_>>().join(","),
        r.capabilities.iter().map(|c| json_string(c)).collect::<Vec<_>>().join(","),
        effects,
        r.effects_executed,
        r.context.time_now,
        json_string(&r.context.random_uuid),
        json_string(&outcome_str(&r.outcome)),
    )
}

/// The header, carrying the device's public key so the token is self-contained.
/// Binding that key to a person is somebody else's job and a separate question —
/// this says which key signed, not whose it is.
pub fn header(device_key: &[u8]) -> String {
    format!(
        "{{\"alg\":\"EdDSA\",\"typ\":\"val-record+jwt\",\"jwk\":{{\"kty\":\"OKP\",\"crv\":\"Ed25519\",\"x\":{}}}}}",
        json_string(&b64(device_key))
    )
}

/// What gets signed: `base64url(header) . base64url(payload)`, per RFC 7515.
pub fn signing_input(r: &ExecutionRecord, device_key: &[u8]) -> String {
    format!("{}.{}", b64(header(device_key).as_bytes()), b64(payload(r).as_bytes()))
}

/// The whole token. A publisher's server takes this and nothing else.
pub fn jwt(r: &ExecutionRecord) -> String {
    format!("{}.{}", signing_input(r, &r.device_key), b64(&r.signature))
}
