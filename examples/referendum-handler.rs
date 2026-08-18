// Runs on the commission's server, not on the phone.
//
// It is handed one execution record and answers with one decision. What it can
// see is deliberately narrow: that an eligible voter's device ran this exact
// code and committed, and which of the two ballot actions it ran. It never sees
// the voter.

use crate::val::{Decision, Sdk};

/// The only two actions a ballot can arrive from. A record naming anything else
/// is a record from a program that is not this ballot, whatever it signed.
const BALLOTS: [&str; 2] = ["VoteYes", "VoteNo"];

pub fn handle(token: &str, val: &Sdk) -> Decision {
    // Signature, code hash, outcome and rollback, all checked before this line.
    let checked = match val.verify(token) {
        Ok(c) => c,
        Err(refusal) => return val.refuse(refusal),
    };

    let action = checked.record["action"].as_str().unwrap_or_default();
    if !BALLOTS.contains(&action) {
        return val.refuse(serde_json::json!({
            "kind": "policy",
            "why": format!("{action} is not a ballot"),
        }));
    }

    // The receipt is signed from what the record shows being issued, never from
    // anything the caller asked for. It carries the question and the time, and
    // nothing about the direction — a receipt that proved which way somebody
    // voted would be a receipt that can be bought.
    match val.issuance(&checked, "BallotReceipt") {
        Some(claims) => val.issue("BallotReceipt", claims),
        None => val.accept("counted, no receipt requested"),
    }
}
