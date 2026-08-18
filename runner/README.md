# The runner

Somewhere a publisher's handler can execute. A handler is given an execution
record and answers with one decision, in TypeScript, Python, Go or Rust.

```bash
cargo run -p valang-runner            # :8787
```

The playground sends a handler here whenever it is not TypeScript, which it
runs in the tab. Point it somewhere else with `VITE_RUNNER`.

## The contract

```
POST /v1/run
{
  "files":     [{ "name": "handler.py", "source": "…" }],
  "entry":     "handler.py",
  "token":     "<the execution record, a compact JWT>",
  "source":    "<the .val package the record claims to have run>",
  "deviceKey": "<hex>",
  "lastRoot":  "<hex, optional>"
}

→ { "kind": "issue",  "credential": "LoyaltyMember", "claims": { … } }
→ { "kind": "accept", "note": "…" }
→ { "kind": "refuse", "refusal": { "kind": "unknownCode", "why": "…" } }
→ { "kind": "threw",  "error": "…" }
```

`POST /v1/languages` lists what it accepts.

## Verification happens once, in Rust

The runner verifies the record itself before any handler runs, and the SDK it
injects returns that result from `verify()`. There is one verifier —
`valang-verify` — rather than four that agree until the day they do not.

A handler that never runs cannot be the thing that decides whether the record
was good, so verification is not something a handler can skip.

## What a handler gets

Its own files, the SDK module beside them, a generated entry point that reads
stdin and prints one JSON decision. Nothing else: the environment is cleared,
the working directory is fresh and removed afterwards, module downloads are off
and there is a wall-clock limit.

| language | entry | runs under |
| --- | --- | --- |
| TypeScript | `handler.ts` | `node --experimental-strip-types` |
| Python | `handler.py` | `python3` |
| Go | `handler.go` | `go run .`, as package `runner/handler` |
| Rust | `handler.rs` | `cargo run --offline`, as `mod handler` |

Node strips the types rather than compiling them, so what runs is the file the
author wrote.

## Deploying

```bash
docker build -f runner/Dockerfile -t $REPO/runner:$TAG .
docker push $REPO/runner:$TAG

cd runner/terraform
terraform init
terraform apply -var project=… -var image=$REPO/runner:$TAG
```

Cloud Run runs each revision in a gVisor sandbox with no ambient credentials,
which is the boundary this service relies on. One request per instance, because
compiling Go or Rust is what the CPU is for and it is bursty.

The service account it runs as is granted nothing. The runner reads no bucket
and calls no API — it is handed a record and answers with a decision.
