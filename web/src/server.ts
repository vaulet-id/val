// The handler a publisher runs.
//
// This is the file somebody edits in the editor, and it is deliberately the
// same shape as the one they would deploy: what arrives is a token, what goes
// back is a decision. The `val` object is the SDK — a thin binding over
// `valang-verify`, which is the same crate a Go or a Python SDK binds to, so
// this is not a second implementation of the check being taught.

/// What a new project's server starts as.
///
/// It issues nothing, because the starter application issues nothing — a
/// handler reaching for an effect that is not in the record would refuse every
/// first press, and the lesson somebody takes from that is that they broke it.
export const STARTER_HANDLER = `// Runs on your server, not on the phone.
//
// The wallet ran an action and signed a record of it. Your job is to decide
// what that is worth. This one only checks the record and accepts it — there is
// nothing to issue yet.

export default async function handler(token, val) {
  // Standard JWS verification, then the checks no standard covers: is this the
  // code you published, did the run commit, did the state go backwards.
  const checked = await val.verify(token)
  if (!checked.ok) return val.refuse(checked.refusal)

  // Nothing to issue, so nothing is signed. Saying so is a decision, and it
  // is the one most handlers make most of the time.
  return val.accept(\`\${checked.record.action} committed, state now \${checked.record.nextRoot.slice(0, 8)}\`)
}
`

export const DEFAULT_HANDLER = `// Runs on your server, not on the phone.
//
// The wallet ran an action and signed a record of it. Your job is to decide
// whether that is grounds to issue a credential — and then to sign it, because
// the application cannot: it has no issuer key, and must not have one.

export default async function handler(token, val) {
  // Standard JWS verification, then the checks no standard covers: is this the
  // code you published, did the run commit, did the state go backwards.
  const checked = await val.verify(token)
  if (!checked.ok) return val.refuse(checked.refusal)

  // What the record shows being issued — never what the client asked you to
  // sign. Signing the request instead would make every check above decorative.
  const claims = val.issuance(checked, 'LoyaltyMember')
  if (!claims) return val.refuse({ kind: 'noSuchEffect', why: 'this run issues nothing' })

  // Your own rules go here. This one is a stand-in for the interesting case:
  // a scheme decides what it will and will not put its name to.
  if (claims.points > 1_000_000) {
    return val.refuse({ kind: 'policy', why: 'that is more points than this scheme issues' })
  }

  return val.issue('LoyaltyMember', claims)
}
`
