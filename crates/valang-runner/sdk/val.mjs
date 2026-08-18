// The SDK a TypeScript or JavaScript handler is given.
//
// Verification already happened, in Rust, before this process started. There is
// one verifier and every language talks to it the same way: the runner hands
// the result in on stdin and this module shapes it.

export function sdk(checked) {
  return {
    verify: async () => checked,

    issuance(c, credential) {
      const e = (c.effects ?? []).find(
        (x) => x.capability === 'credential.issue' && x.payload?.credential === credential,
      )
      return e?.payload?.claims ?? null
    },

    issue: (credential, claims) => ({ kind: 'issue', credential, claims }),
    accept: (note) => ({ kind: 'accept', note }),
    refuse: (refusal) => ({ kind: 'refuse', refusal }),
  }
}

export async function main(handle) {
  const chunks = []
  for await (const chunk of process.stdin) chunks.push(chunk)
  const payload = JSON.parse(Buffer.concat(chunks).toString())
  const decision = await handle(payload.token, sdk(payload.checked))
  process.stdout.write(JSON.stringify(decision))
}
