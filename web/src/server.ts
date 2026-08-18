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

/// The same handler, in each language the server group accepts.
///
/// The contract does not change between them: you are handed the execution
/// record as a JWT, you verify it, and you return one decision. What differs is
/// only the syntax, because the record is a signed JWT with a published `vct`
/// and every language has a JOSE library.
export const HANDLERS: Record<string, string> = {
  ts: `// Runs on your server, not on the phone.

export default async function handler(token, val) {
  const checked = await val.verify(token)
  if (!checked.ok) return val.refuse(checked.refusal)

  const claims = val.issuance(checked, 'LoyaltyMember')
  if (!claims) return val.accept('nothing to issue')

  return val.issue('LoyaltyMember', claims)
}
`,

  go: `// Runs on your server, not on the phone.

package handler

import "runner/val"

func Handle(token string, v val.SDK) val.Decision {
	checked, err := v.Verify(token)
	if err != nil {
		return v.Refuse(err)
	}

	claims, ok := v.Issuance(checked, "LoyaltyMember")
	if !ok {
		return v.Accept("nothing to issue")
	}

	return v.Issue("LoyaltyMember", claims)
}
`,

  rs: `// Runs on your server, not on the phone.

use val::{Decision, Sdk};

pub fn handle(token: &str, val: &Sdk) -> Decision {
    let checked = match val.verify(token) {
        Ok(c) => c,
        Err(refusal) => return val.refuse(refusal),
    };

    match val.issuance(&checked, "LoyaltyMember") {
        Some(claims) => val.issue("LoyaltyMember", claims),
        None => val.accept("nothing to issue"),
    }
}
`,

  py: `# Runs on your server, not on the phone.

def handle(token, val):
    checked = val.verify(token)
    if not checked.ok:
        return val.refuse(checked.refusal)

    claims = val.issuance(checked, "LoyaltyMember")
    if claims is None:
        return val.accept("nothing to issue")

    return val.issue("LoyaltyMember", claims)
`,
}

/// What a new server file starts as, by extension. A file that is not the
/// entry point starts as a module, since the entry point is what imports it.
export function serverStarter(name: string): string {
  const ext = name.split('.').pop() ?? 'ts'
  if (name.startsWith('handler.')) return HANDLERS[ext] ?? HANDLERS.ts

  const modules: Record<string, string> = {
    ts: `export function help() {\n  return 'help'\n}\n`,
    go: `package handler\n\nfunc Help() string {\n\treturn "help"\n}\n`,
    rs: `pub fn help() -> &'static str {\n    "help"\n}\n`,
    py: `def help():\n    return "help"\n`,
  }
  return modules[ext] ?? modules.ts
}
