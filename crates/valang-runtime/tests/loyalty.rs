//! Running the loyalty card, against numbers this crate did not produce.
//!
//! The expected values are pinned from the playground's TypeScript simulator,
//! which walked the same source with an independently written evaluator before
//! this one existed. Two implementations agreeing is worth something; this
//! crate agreeing with itself would not be.

use std::collections::BTreeMap;

use valang_runtime::canonical::{Canonical, DeterministicCbor};
use valang_runtime::host::{Context, EffectRequest, Host, Verdict};
use valang_runtime::value::Value;
use valang_runtime::{run_action, Outcome};

const LOYALTY: &str = include_str!("../../../examples/loyalty.val");
const FIXTURE: &str = include_str!("../../../fixtures/wallet.json");

/// The same wallet the playground shows and `valrun` uses. Three separate
/// inventions of "what is on this phone" meant three answers and no way to tell
/// which one a bug was about.
fn fixture() -> valang_runtime::fixture::Fixture {
    valang_runtime::fixture::Fixture::parse(FIXTURE).expect("the fixture parses")
}

struct Wallet {
    approve: bool,
}

impl Host for Wallet {
    fn context(&self) -> Context {
        Context { time_now: 1_755_426_600_000, random_uuid: "0f2a-c71b".into() }
    }

    fn credential(&self, ty: &str, _policy: Option<&str>) -> Option<BTreeMap<String, Value>> {
        let mut c = BTreeMap::new();
        if ty == "NationalId" {
            c.insert("country".into(), Value::Str("TH".into()));
            c.insert("birthdate".into(), Value::Int(820_454_400_000));
            return Some(c);
        }
        if ty != "PurchaseReceipt" {
            return None;
        }
        c.insert("merchant".into(), Value::Str("Codefin Coffee".into()));
        c.insert("amount".into(), Value::Int(12_500)); // satang
        c.insert("purchased_at".into(), Value::Int(1_755_335_520_000));
        Some(c)
    }

    fn decide(&self, _effects: &[EffectRequest]) -> Verdict {
        if self.approve {
            Verdict::Approved
        } else {
            Verdict::Refused("the person said no".into())
        }
    }
    fn sign(&self, bytes: &[u8]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes).to_vec()
    }
    fn device_key(&self) -> Vec<u8> {
        b"test-device".to_vec()
    }
}

fn member(points: i64, tier: &str) -> Value {
    let mut m = BTreeMap::new();
    m.insert("member_id".into(), Value::Str("M-2891".into()));
    m.insert("points".into(), Value::Int(points));
    m.insert("tier".into(), Value::Enum("Tier".into(), tier.into()));
    Value::Map(m)
}

fn start() -> BTreeMap<String, Value> {
    let mut s = BTreeMap::new();
    s.insert("lifetimePoints".into(), Value::Int(1_240));
    s.insert("member".into(), member(1_240, "bronze"));
    s
}

#[test]
fn a_scan_earns_one_point_per_baht_and_commits() {
    let (program, diagnostics) = valang::analyse(LOYALTY);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let run = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: true });
    assert_eq!(run.outcome, Outcome::Committed);

    // 12_500 satang / 100 = 125 points, on top of 1_240.
    assert_eq!(run.next_state["lifetimePoints"], Value::Int(1_365));
    let m = run.next_state["member"].field("points").cloned();
    assert_eq!(m, Some(Value::Int(1_365)));

    assert_eq!(run.effects.len(), 1);
    assert_eq!(run.effects[0].capability, "credential.issue");
    assert_ne!(run.record.previous_root, run.record.next_root);

    // What is in the credential, not only that one was asked for. The example
    // fills these from `next`, and checking the capability alone let a version
    // that issued three nulls pass.
    let Value::Credential { ty, claims, .. } = &run.effects[0].payload else {
        panic!("a credential was expected, found {}", run.effects[0].payload)
    };
    assert_eq!(ty, "LoyaltyMember");
    assert_eq!(claims["points"], Value::Int(1_365));
    assert_eq!(claims["member_id"], Value::Str("M-2891".into()));
    assert_eq!(claims["tier"], Value::Enum("Tier".into(), "bronze".into()));
}

#[test]
fn a_refused_batch_commits_nothing() {
    let (program, _) = valang::analyse(LOYALTY);
    let before = start();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &before, &BTreeMap::new(), &Wallet { approve: false });

    assert!(matches!(run.outcome, Outcome::Refused(_)));
    assert_eq!(run.next_state, before, "the state must not move when the batch did not");
    assert_eq!(run.record.previous_root, run.record.next_root);
    assert_eq!(run.record.effects_executed, 0);
    // The request is still in the record: what was asked for is part of what
    // happened, whether or not it was granted.
    assert_eq!(run.record.effects_requested.len(), 1);
}

#[test]
fn the_same_state_encodes_to_the_same_bytes_whatever_order_it_was_built_in() {
    let enc = DeterministicCbor;
    let mut a = BTreeMap::new();
    a.insert("b".to_string(), Value::Int(2));
    a.insert("a".to_string(), Value::Int(1));
    let mut b = BTreeMap::new();
    b.insert("a".to_string(), Value::Int(1));
    b.insert("b".to_string(), Value::Int(2));
    assert_eq!(enc.encode(&Value::Map(a)), enc.encode(&Value::Map(b)));
}

/// A proof hands the host a statement, never the answer. Evaluating the
/// predicate here and shipping the boolean would be a disclosure wearing the
/// word `prove` — the failure mode §5 says the compiler must refuse.
#[test]
fn a_proof_carries_the_statement_and_not_its_value() {
    const DOOR: &str = include_str!("../../../examples/door.val");
    let (program, diagnostics) = valang::analyse(DOOR);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let run = run_action(&program, DOOR, "EnterVenue", &BTreeMap::new(), &BTreeMap::new(), &Wallet { approve: true });
    assert_eq!(run.effects.len(), 1, "one batch, one disclosure");
    let e = &run.effects[0];
    assert_eq!(e.capability, "disclosure.present");
    assert!(!e.reversible, "nothing un-tells somebody a postcode");

    let shown = e.payload.to_string();
    assert!(shown.contains("statement:"), "the predicate travels: {shown}");
    assert!(
        !shown.contains("kind: \"prove\", of:"),
        "the answer must not: {shown}"
    );
}

/// The property the whole scheme rests on: one value, one encoding, and two
/// different values never the same bytes. An enum member used to encode as a
/// bare pair of strings, which made `Tier.gold` and the list `["Tier", "gold"]`
/// indistinguishable — and a hash over bytes stops being a hash over a value
/// the moment that is true.
#[test]
fn two_different_values_are_never_the_same_bytes() {
    use valang_runtime::decode::decode;
    let enc = DeterministicCbor;

    let member = Value::Enum("Tier".into(), "gold".into());
    let pair = Value::List(vec![Value::Str("Tier".into()), Value::Str("gold".into())]);
    assert_ne!(enc.encode(&member), enc.encode(&pair));

    for v in [member, pair, Value::Int(-1_000_000), Value::Str("แต้ม".into()), Value::Bool(true), Value::Null] {
        assert_eq!(decode(&enc.encode(&v)).unwrap(), v, "round trip: {v}");
    }
}

/// A strict decoder is what makes re-encoding sound. Without it a package could
/// arrive encoded some other legal way, and a verifier that re-encodes would be
/// checking a signature against its own idea of the file.
#[test]
fn a_non_canonical_encoding_is_refused() {
    use valang_runtime::decode::{decode, Malformed};

    // 10, written in two bytes instead of one. Legal CBOR, not this encoding.
    assert!(matches!(decode(&[0x18, 0x0a]), Err(Malformed::NotShortest { .. })));
    // A map whose keys are out of order.
    let mut unsorted = vec![0xa2];
    unsorted.extend_from_slice(&[0x61, b'b', 0x01]);
    unsorted.extend_from_slice(&[0x61, b'a', 0x02]);
    assert!(matches!(decode(&unsorted), Err(Malformed::KeysNotSorted { .. })));
    // Trailing bytes after a complete value.
    assert!(matches!(decode(&[0x01, 0x01]), Err(Malformed::Trailing { .. })));
}

/// Test vectors from RFC 8949 appendix A, which is the point: an encoder
/// checked against its own idea of the format checks nothing.
#[test]
fn the_encoding_is_the_one_in_the_rfc() {
    let enc = DeterministicCbor;
    for (value, bytes) in [
        (Value::Int(0), vec![0x00]),
        (Value::Int(1), vec![0x01]),
        (Value::Int(10), vec![0x0a]),
        (Value::Int(23), vec![0x17]),
        (Value::Int(24), vec![0x18, 0x18]),
        (Value::Int(100), vec![0x18, 0x64]),
        (Value::Int(1000), vec![0x19, 0x03, 0xe8]),
        (Value::Int(1_000_000), vec![0x1a, 0x00, 0x0f, 0x42, 0x40]),
        (Value::Int(-1), vec![0x20]),
        (Value::Int(-10), vec![0x29]),
        (Value::Int(-100), vec![0x38, 0x63]),
        (Value::Bool(false), vec![0xf4]),
        (Value::Bool(true), vec![0xf5]),
        (Value::Null, vec![0xf6]),
        (Value::Str(String::new()), vec![0x60]),
        (Value::Str("a".into()), vec![0x61, 0x61]),
        (Value::Str("IETF".into()), vec![0x64, 0x49, 0x45, 0x54, 0x46]),
        (Value::List(vec![]), vec![0x80]),
        (
            Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            vec![0x83, 0x01, 0x02, 0x03],
        ),
    ] {
        assert_eq!(enc.encode(&value), bytes, "encoding {value}");
    }
}

/// The reason state is a tree. A verifier is handed one field and the root it
/// already had, and can check the field belongs to that state without being
/// shown any of the rest of it.
#[test]
fn one_field_can_be_proved_without_opening_the_others() {
    use valang_runtime::merkle::{leaves, prove, root, verify_inclusion};

    let (program, _) = valang::analyse(LOYALTY);
    let run = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: true });

    let enc = DeterministicCbor;
    let ls = leaves(&run.next_state, &enc);
    let r = root(&ls);
    assert_eq!(r, run.record.next_root, "the record's root is this tree's root");

    for leaf in &ls {
        let p = prove(&ls, &leaf.path).expect("every leaf is provable");
        assert!(verify_inclusion(&p, &r, &enc), "{} should verify", leaf.path);
        // And it is about that field only: the proof carries no other value.
        assert_eq!(p.path, leaf.path);
        assert_eq!(p.value, leaf.value);
    }

    // A field claimed with a value it does not have must not verify.
    let mut lying = prove(&ls, "member.points").unwrap();
    lying.value = Value::Int(999_999);
    assert!(!verify_inclusion(&lying, &r, &enc));

    // Nor an honest proof against a different state's root: a proof is about
    // one state, which is what makes a remembered root worth remembering.
    let other = root(&leaves(&start(), &enc));
    let honest = prove(&ls, "member.points").unwrap();
    assert!(!verify_inclusion(&honest, &other, &enc));
}

/// A refused run is signed too. The record is evidence of what happened, and
/// "the host would not take this batch" is something that happened — a record
/// that only attested to successes would be evidence of a different thing than
/// it claims to be.
#[test]
fn the_record_is_signed_whichever_way_the_run_went() {
    let (program, _) = valang::analyse(LOYALTY);
    for approve in [true, false] {
        let run = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve });
        assert!(!run.record.signature.is_empty(), "unsigned with approve={approve}");
        assert_eq!(run.record.device_key, b"test-device".to_vec());

        // The outcome is inside the signed bytes, so the two runs cannot be
        // swapped for one another — and what is signed is the JWS signing input,
        // so a publisher checks it with an ordinary JWT library.
        let input = valang_runtime::attestation::signing_input(&run.record, &run.record.device_key);
        assert_eq!(Wallet { approve }.sign(input.as_bytes()), run.record.signature);
        assert_eq!(valang_runtime::attestation::jwt(&run.record).split('.').count(), 3);
    }

    let a = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: true });
    let b = run_action(&program, LOYALTY, "ScanToEarn", &start(), &BTreeMap::new(), &Wallet { approve: false });
    assert_ne!(a.record.signature, b.record.signature);
}

/// A screen declares and the host resolves — before anything is drawn, which is
/// why there is no half-drawn screen and no prompt arriving mid-scroll. What
/// comes back is a description for the host's toolkit, with the grades decided
/// here rather than by the application, which has an interest in the answer.
#[test]
fn a_screen_is_resolved_by_the_host_before_it_is_drawn() {
    use valang_runtime::render::render;

    // The screen is the second file of the loyalty package, so it is analysed
    // with the first — alone it presses an action that is not there.
    const WALLET: &str = include_str!("../../../examples/wallet.val");
    let package = format!("{LOYALTY}\n{WALLET}");
    let (program, diagnostics) = valang::analyse(&package);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    struct Shop;
    impl Host for Shop {
        fn context(&self) -> Context {
            Context { time_now: 0, random_uuid: String::new() }
        }
        fn credential(&self, _ty: &str, _p: Option<&str>) -> Option<BTreeMap<String, Value>> {
            None
        }
        fn decide(&self, _e: &[EffectRequest]) -> Verdict {
            Verdict::Approved
        }
        fn sign(&self, _b: &[u8]) -> Vec<u8> {
            Vec::new()
        }
        fn device_key(&self) -> Vec<u8> {
            Vec::new()
        }
        fn credentials_of(&self, ty: &str, policy: Option<&str>, limit: Option<i64>) -> Vec<BTreeMap<String, Value>> {
            assert_eq!(ty, "PurchaseReceipt");
            assert_eq!(policy, Some("ReceiptFromMerchant"), "the policy the screen named travels with the request");
            assert_eq!(limit, Some(50), "and so does the bound that makes it finite");
            (0..2)
                .map(|i| {
                    BTreeMap::from([
                        ("merchant".to_string(), Value::Str(format!("Shop {i}"))),
                        ("amount".to_string(), Value::Int(10_000)),
                        ("purchased_at".to_string(), Value::Int(1_700_000_000_000)),
                    ])
                })
                .collect()
        }
    }

    let screen = render(&program, "Wallet", &BTreeMap::new(), &Shop).expect("renders");
    assert_eq!(screen.name, "Wallet");
    assert_eq!(screen.data.len(), 1);
    assert_eq!(screen.data[0].grade, "issuer", "a policy was named, so an issuer stands behind it");
    assert_eq!(screen.data[0].rows, 2);
    assert!(!screen.tree.is_empty());

    // `onTap` carries an action's name, not a value: evaluating it would look
    // for something that is not there.
    fn find<'a>(cs: &'a [valang_runtime::render::Component], kind: &str) -> Option<&'a valang_runtime::render::Component> {
        for c in cs {
            if c.kind == kind {
                return Some(c);
            }
            if let Some(found) = find(&c.children, kind) {
                return Some(found);
            }
        }
        None
    }
    let button = find(&screen.tree, "button").expect("the screen has a button");
    assert_eq!(button.args.get("onTap"), Some(&Value::Str("ScanToEarn".into())));
}

/// Totality bounds steps and not memory, so the sizes are the host's to carry
/// and its to refuse. Checked before the state commits rather than while it is
/// being built: a limit that stopped an action halfway would leave a state no
/// phase produced.
#[test]
fn a_state_too_large_for_this_host_does_not_commit() {
    use valang_runtime::host::Limits;

    let src = r#"
app "example.big"
version 1
capabilities { }
state { note: string default "" }
action Grow {
  update { note: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
}
"#;
    let (program, diagnostics) = valang::analyse(src);
    assert!(diagnostics.iter().all(|d| d.severity != valang::Severity::Error), "{diagnostics:?}");

    struct Tiny;
    impl Host for Tiny {
        fn context(&self) -> Context {
            Context { time_now: 0, random_uuid: String::new() }
        }
        fn limits(&self) -> Limits {
            Limits { max_list: 4, max_string_bytes: 8, max_state_bytes: 64 }
        }
        fn credential(&self, _t: &str, _p: Option<&str>) -> Option<BTreeMap<String, Value>> {
            None
        }
        fn decide(&self, _e: &[EffectRequest]) -> Verdict {
            Verdict::Approved
        }
        fn sign(&self, _b: &[u8]) -> Vec<u8> {
            Vec::new()
        }
        fn device_key(&self) -> Vec<u8> {
            Vec::new()
        }
    }

    let before = BTreeMap::from([("note".to_string(), Value::Str(String::new()))]);
    let run = run_action(&program, src, "Grow", &before, &BTreeMap::new(), &Tiny);

    match &run.outcome {
        Outcome::Defect(why) => assert!(why.contains("bytes of text"), "{why}"),
        other => panic!("a state this host cannot carry should not commit, got {other:?}"),
    }
    assert_eq!(run.next_state, before, "and nothing moved");
    assert_eq!(run.record.previous_root, run.record.next_root);

    // The same program under a host that can carry it commits.
    let ok = run_action(&program, src, "Grow", &before, &BTreeMap::new(), &Wallet { approve: true });
    assert_eq!(ok.outcome, Outcome::Committed);
}

/// Declining is an ordinary outcome and not a defect: nothing went wrong, the
/// state does not move, and what the person is shown is a key somebody signed
/// rather than a sentence the application assembled.
#[test]
fn a_declined_run_commits_nothing_and_names_what_to_show() {
    struct SmallPurchase;
    impl Host for SmallPurchase {
        fn context(&self) -> Context {
            Context { time_now: 1_755_426_600_000, random_uuid: "0f2a-c71b".into() }
        }
        fn credential(&self, _ty: &str, _p: Option<&str>) -> Option<BTreeMap<String, Value>> {
            Some(BTreeMap::from([
                ("merchant".to_string(), Value::Str("Codefin Coffee".into())),
                // Under the twenty baht the example declines below.
                ("amount".to_string(), Value::Int(1_500)),
                ("purchased_at".to_string(), Value::Int(1_755_335_520_000)),
            ]))
        }
        fn decide(&self, _e: &[EffectRequest]) -> Verdict {
            panic!("the host is never asked: the application declined before there was a batch")
        }
        fn sign(&self, bytes: &[u8]) -> Vec<u8> {
            use sha2::{Digest, Sha256};
            Sha256::digest(bytes).to_vec()
        }
        fn device_key(&self) -> Vec<u8> {
            b"test-device".to_vec()
        }
    }

    let (program, _) = valang::analyse(LOYALTY);
    let before = start();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &before, &BTreeMap::new(), &SmallPurchase);

    match &run.outcome {
        Outcome::Declined(key) => assert_eq!(key, "tooSmallToEarn"),
        other => panic!("expected a decline, got {other:?}"),
    }
    assert_eq!(run.next_state, before);
    assert!(run.effects.is_empty(), "there was never a batch");
    assert!(!run.record.signature.is_empty(), "and it is still a record of what happened");
}

/// The fixture is the wallet, and a run over it agrees with the one over the
/// hand-written host above — which is the only reason to have one file instead
/// of three.
#[test]
fn the_fixture_is_the_same_wallet_the_tests_were_writing_by_hand() {
    let (program, _) = valang::analyse(LOYALTY);
    let host = fixture();

    let run = run_action(&program, LOYALTY, "ScanToEarn", &host.state(), &BTreeMap::new(), &host);
    assert_eq!(run.outcome, Outcome::Committed);
    assert_eq!(run.next_state["lifetimePoints"], Value::Int(1_365));

    // Times in the file are ISO-8601 and integers here, because that is what
    // comparing one to `context.time.now` needs — a string compares false and
    // says nothing, which is how the freshness rule in every trust policy would
    // have quietly stopped working.
    let host = fixture();
    let receipts = host.credentials_of("PurchaseReceipt", Some("ReceiptFromMerchant"), Some(2));
    assert_eq!(receipts.len(), 2, "the declaration's limit bounds what the host hands back");
    assert!(matches!(receipts[0]["purchased_at"], Value::Int(_)));

    let refused = fixture().refusing();
    let run = run_action(&program, LOYALTY, "ScanToEarn", &refused.state(), &BTreeMap::new(), &refused);
    assert!(matches!(run.outcome, Outcome::Refused(_)));
}

/// Encoding and decoding have to be inverses, for every value the language has.
/// Two have needed a tag to make that true — an enum member, which was a pair of
/// strings, and a credential, which was a map anybody could also have written by
/// hand. Both were found by something downstream reading back subtly other than
/// what was signed.
#[test]
fn every_value_survives_the_round_trip() {
    use valang_runtime::decode::decode;
    let enc = DeterministicCbor;

    let values = vec![
        Value::Null,
        Value::Bool(true),
        Value::Int(-1_000_000),
        Value::Str("แต้ม".into()),
        Value::Bytes(vec![0, 1, 2, 255]),
        Value::Enum("Tier".into(), "gold".into()),
        Value::List(vec![Value::Str("Tier".into()), Value::Str("gold".into())]),
        Value::Map(BTreeMap::from([("a".to_string(), Value::Int(1))])),
        Value::Credential {
            ty: "LoyaltyMember".into(),
            claims: BTreeMap::from([("points".to_string(), Value::Int(1_365))]),
            verified: Some("ReceiptFromMerchant".into()),
        },
        // The shape a credential used to encode as, which must not decode as one.
        Value::Map(BTreeMap::from([
            ("type".to_string(), Value::Str("LoyaltyMember".into())),
            ("claims".to_string(), Value::Map(BTreeMap::new())),
        ])),
    ];

    for v in &values {
        assert_eq!(decode(&enc.encode(v)).unwrap(), *v, "round trip: {v}");
    }

    // And no two of them encode alike.
    let mut bytes: Vec<Vec<u8>> = values.iter().map(|v| enc.encode(v)).collect();
    let before = bytes.len();
    bytes.sort();
    bytes.dedup();
    assert_eq!(bytes.len(), before, "two different values encoded to the same bytes");
}
