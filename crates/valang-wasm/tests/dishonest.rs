//! A publisher who lies, and what stops them.
//!
//! The modules here are **written by hand, not compiled**. That is the whole
//! point: a publisher compiles on their own machine, so nothing about their
//! module having come from a VAL source can be assumed. Every check a wallet
//! makes has to hold against bytes an attacker chose.

use wasm_encoder::{
    CodeSection, EntityType, ExportSection, Function, FunctionSection, ImportSection, Instruction,
    Module, TypeSection, ValType,
};

/// A module that imports what it likes and calls it from one exported action.
fn forged(imports: &[(&str, &str, usize)], calls: &[usize]) -> Vec<u8> {
    let mut types = TypeSection::new();
    for arity in 0..=3usize {
        types.ty().function(vec![ValType::I32; arity], vec![ValType::I32]);
    }
    let mut section = ImportSection::new();
    for (ns, name, arity) in imports {
        section.import(ns, name, EntityType::Function(*arity as u32));
    }
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    let mut exports = ExportSection::new();
    exports.export("action:Go", wasm_encoder::ExportKind::Func, imports.len() as u32);

    let mut body = Function::new(vec![]);
    for i in calls {
        body.instruction(&Instruction::Call(*i as u32));
        body.instruction(&Instruction::Drop);
    }
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);

    let mut m = Module::new();
    m.section(&types);
    m.section(&section);
    m.section(&funcs);
    m.section(&exports);
    m.section(&code);
    m.finish()
}

/// **Reaching for something and not saying so.** There is nothing to stop a
/// module importing whatever it likes — so the answer is not to stop it, it is
/// that whatever it imports is what the person is shown.
#[test]
fn what_it_reaches_for_is_what_the_sheet_says() {
    let bytes = forged(
        &[("cap", "read:NationalId.birthdate", 0), ("cap", "issue:Anything", 1)],
        &[],
    );
    let wants = valang_wasm::wants_of(&bytes).expect("a module this host can describe");
    assert!(wants.reads.contains("NationalId.birthdate"), "{wants:?}");
    assert!(wants.issues.contains("Anything"), "{wants:?}");
}

/// **Reaching outside the ABI.** The refusal that makes every other line mean
/// something.
#[test]
fn reaching_outside_the_abi_is_refused() {
    for import in [
        ("wasi_snapshot_preview1", "fd_write", 1),
        ("env", "memory_grow", 1),
        ("cap", "read_everything", 0),
        ("val", "exec", 1),
    ] {
        let bytes = forged(&[import], &[]);
        assert!(
            valang_wasm::wants_of(&bytes).is_err(),
            "a module importing {}/{} was accepted",
            import.0,
            import.1
        );
    }
}

/// **The one that matters.** A module asks for a whole credential *and* one
/// claim of it. The sheet renders the claims under their policy — "reads the
/// country on your national ID" — and the credential handle it was also given
/// would have carried the birthdate, the name and the document number with it.
///
/// The sheet would have been true about the import list and false about what
/// the module holds.
#[test]
fn a_credential_hands_over_only_the_claims_that_were_asked_for() {
    let bytes = forged(
        &[
            ("cap", "read:NationalId under GovernmentIssued", 0),
            ("cap", "read:NationalId.country", 0),
        ],
        &[],
    );
    let wants = valang_wasm::wants_of(&bytes).expect("describable");

    // What a person is shown.
    let shown = wants.reads_as_lines();
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert!(shown.iter().next().unwrap().contains("country"), "{shown:?}");
    assert!(
        !shown.iter().next().unwrap().contains("birthdate"),
        "the sheet does not mention the birthdate: {shown:?}"
    );

    // So the credential it is handed must carry the country and nothing else.
    let held = valang_wasm::engine::claims_handed_over(
        &bytes,
        "NationalId",
        &["country".into(), "birthdate".into(), "family_name".into()],
    );
    assert_eq!(
        held,
        vec!["country".to_string()],
        "the module was handed claims the sheet never mentioned"
    );
}

/// **A loop.** Totality is a property of programs this compiler emitted, and a
/// publisher does not have to use it. Without a fuel budget the phone stops,
/// and the person is looking at a wallet that hung rather than an application
/// that misbehaved.
#[test]
fn a_module_that_never_finishes_is_stopped() {
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    let mut exports = ExportSection::new();
    exports.export("action:Go", wasm_encoder::ExportKind::Func, 0);

    // `loop { br 0 }` — the shortest program that does not end.
    let mut body = Function::new(vec![]);
    body.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Br(0));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);

    let mut m = Module::new();
    m.section(&types);
    m.section(&funcs);
    m.section(&exports);
    m.section(&code);
    let bytes = m.finish();

    let module = valang_wasm::compile::Module {
        bytes,
        konsts: Vec::new(),
        functions: Vec::new(),
    };
    let host = valang_runtime::fixture::Fixture::parse(include_str!(
        "../../../fixtures/wallet.json"
    ))
    .expect("the wallet parses");
    let about = valang_runtime::About {
        app: "x.y".into(),
        version: "1".into(),
        actions: vec![valang_runtime::ActionAbout { name: "Go".into(), inputs: Vec::new() }],
        ..Default::default()
    };
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let run = valang_runtime::run_action_with(
        &about,
        [0u8; 32],
        "Go",
        &Default::default(),
        &Default::default(),
        &host,
        &mut engine,
    );
    assert!(
        matches!(&run.outcome, valang_runtime::Outcome::Failed(why) if why.contains("longer than it was given")),
        "a module that never finishes was not stopped: {:?}",
        run.outcome
    );
}

/// **Metadata that understates.** The capability list goes into every execution
/// record, and a module nobody compiled carries whatever metadata its author
/// wrote. It is measured from the import section instead, so a record cannot
/// say less than the module can reach.
#[test]
fn the_capability_list_is_measured_and_not_believed() {
    use std::collections::BTreeMap;
    use valang_runtime::canonical::{Canonical, DeterministicCbor};
    use valang_runtime::value::Value;

    // Metadata written by hand, claiming this application does nothing at all.
    let mut said = BTreeMap::new();
    said.insert("app".to_string(), Value::Str("x.y".into()));
    said.insert("version".to_string(), Value::Str("1".into()));
    said.insert("capabilities".to_string(), Value::List(Vec::new()));
    said.insert("policies".to_string(), Value::List(Vec::new()));
    said.insert("actions".to_string(), Value::List(Vec::new()));
    let section = DeterministicCbor.encode(&Value::Map(said));

    let mut bytes = forged(&[("cap", "issue:Anything", 1), ("cap", "read:R.x", 0)], &[]);
    let mut with_lie = Module::new();
    with_lie.section(&wasm_encoder::RawSection {
        id: 0,
        data: &{
            // A custom section is its name, length-prefixed, then its bytes.
            let name = valang_wasm::compile::ABOUT_SECTION.as_bytes();
            let mut out = Vec::new();
            out.push(name.len() as u8);
            out.extend_from_slice(name);
            out.extend_from_slice(&section);
            out
        },
    });
    let head = std::mem::take(&mut bytes);
    let mut all = head[..8].to_vec();
    all.extend_from_slice(&with_lie.finish()[8..]);
    all.extend_from_slice(&head[8..]);

    let about = valang_wasm::compile::about_of(&all).expect("describable");
    assert_eq!(about.app, "x.y", "the section it carries is read");
    assert!(about.capabilities.contains(&"credential.issue".to_string()), "{:?}", about.capabilities);
    assert!(about.capabilities.contains(&"credential.read".to_string()), "{:?}", about.capabilities);
}

/// **Disclosing something other than what the sheet names.** The sheet said the
/// country; the module passed the birthdate. It could, because `disclose` used
/// to take a value and the name it was imported under was decoration.
///
/// It takes nothing now. The host looks up the claim the import names and hands
/// it to whoever is being shown it — the module never holds it, so it cannot
/// substitute one, keep it, or compute on it.
#[test]
fn a_disclosure_is_the_claim_the_sheet_names() {
    assert_eq!(
        valang_wasm::Cap::Disclose("NationalId.country".into()).arity(),
        0,
        "a module that hands over the value is a module that chooses it"
    );
    assert_eq!(valang_wasm::Cap::Present.arity(), 0, "and it does not compose the list either");
}

/// **A credential arriving whole through the back door.** `val.input:receipt`
/// is not a line in the report — it is the host handing over what somebody
/// chose — and it used to carry every claim on the credential. A module could
/// read a person's whole national ID with nothing said about it anywhere.
#[test]
fn an_input_credential_carries_only_what_was_named() {
    let bytes = forged(&[("val", "input:id", 0), ("cap", "read:NationalId.country", 0)], &[]);
    let held = valang_wasm::engine::claims_handed_over(
        &bytes,
        "NationalId",
        &["country".into(), "birthdate".into(), "family_name".into()],
    );
    assert_eq!(held, vec!["country".to_string()], "a whole credential came in through `input:`");
}

/// **Payments.** A module passes the amount and the sheet renders the arguments
/// the author wrote, so the two can differ. There is no design for that yet, so
/// a module that asks is refused rather than run against a sheet that describes
/// a different number.
#[test]
fn a_payment_is_refused_rather_than_described_wrongly() {
    let bytes = forged(&[("cap", "pay:amount: total", 1)], &[]);
    // The sheet can still describe it — it is the running that is refused.
    let wants = valang_wasm::wants_of(&bytes).expect("describable");
    assert!(!wants.payments.is_empty());

    let module =
        valang_wasm::compile::Module { bytes, konsts: Vec::new(), functions: Vec::new() };
    let host = valang_runtime::fixture::Fixture::parse(include_str!(
        "../../../fixtures/wallet.json"
    ))
    .expect("the wallet parses");
    let about = valang_runtime::About {
        app: "x.y".into(),
        version: "1".into(),
        actions: vec![valang_runtime::ActionAbout { name: "Go".into(), inputs: Vec::new() }],
        ..Default::default()
    };
    let mut engine = valang_wasm::WasmEngine::new(&module);
    let run = valang_runtime::run_action_with(
        &about,
        [0u8; 32],
        "Go",
        &Default::default(),
        &Default::default(),
        &host,
        &mut engine,
    );
    assert!(
        matches!(&run.outcome, valang_runtime::Outcome::Defect(why) if why.contains("payments")),
        "{:?}",
        run.outcome
    );
}
