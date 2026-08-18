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

## Where the boundary is

**A handler runs as a process inside the runner's own machine.** Around it: a
cleared environment, a fresh directory removed afterwards, no module downloads,
and a wall-clock limit. Around that: the machine.

So the machine is the boundary, and handlers from different publishers share
one. That is the honest description, and it is why the machine is a Firecracker
microVM rather than a container on a shared kernel.

Per-run isolation — one microVM created and destroyed around a single handler —
is a different design, and it costs a VM start per request rather than a process
start. Nothing above `sandbox.rs` would change: the contract there is a process
with stdin and stdout.

## Deploying

Fly, which is where everything else runs, and whose machines are Firecracker:

```bash
fly deploy -c runner/fly.toml
fly volumes create runner_cache --app val-runner --region sin --size 3
```

The volume holds the Go and Rust build caches. Without it a handler compiles
from cold on every request, which is most of what those two languages cost:
1.9s against 0.55s for Go, 1.8s against 0.40s for Rust.

Google Cloud Run is provisioned in `terraform/` for the day this needs to be
somewhere else. It sandboxes with gVisor rather than a VM, which is a weaker
boundary for the same job.

```bash
docker build -f runner/Dockerfile -t $REPO/runner:$TAG .
docker push $REPO/runner:$TAG

cd runner/terraform
terraform init
terraform apply -var project=… -var image=$REPO/runner:$TAG
```
