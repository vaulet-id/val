import loyalty from '../../examples/loyalty.val?raw'
import portfolio from '../../examples/portfolio.val?raw'
import wallet from '../../examples/wallet.val?raw'
import door from '../../examples/door.val?raw'
import rejected from '../../examples/rejected.val?raw'
import textBundle from '../../examples/text.json?raw'
import walletFixture from '../../fixtures/wallet.json?raw'
import { DEFAULT_HANDLER } from './server'

import spec from '../../docs/spec.md?raw'
import readme from '../../README.md?raw'
import examplesReadme from '../../examples/README.md?raw'

/// `pkg` is which package a file belongs to. A package is several files sharing
/// one scope, so the playground analyses them together — `wallet.val` presses an
/// action `loyalty.val` declares, and either alone is half a program.
export type SourceFile = { path: string; name: string; source: string; note: string; pkg: string }

// Read out of the repository rather than copied into the playground. A copy is
// how a playground starts teaching a language that no longer exists.
export const files: SourceFile[] = [
  { path: 'examples/loyalty.val', pkg: 'loyalty', name: 'loyalty.val', source: loyalty, note: 'every phase, both verifiability types' },
  { path: 'examples/portfolio.val', pkg: 'portfolio', name: 'portfolio.val', source: portfolio, note: 'no state, no issuance, a proof that discloses nothing' },
  { path: 'examples/wallet.val', pkg: 'loyalty', name: 'wallet.val', source: wallet, note: 'a screen — the second file of the loyalty package' },
  { path: 'examples/door.val', pkg: 'door', name: 'door.val', source: door, note: 'prove an age, disclose nothing' },
  { path: 'examples/rejected.val', pkg: 'rejected', name: 'rejected.val', source: rejected, note: 'programs that must not compile' },
  { path: 'examples/text.json', pkg: 'loyalty', name: 'text.json', source: textBundle, note: 'the signed text bundle' },
]

/// Not part of any package: a `.va` never carries somebody's wallet. It is the
/// host's answers — what a screen's declaration resolves to — and it is here so
/// that editing it changes what the preview shows and what a run computes.
export const hostFiles: SourceFile[] = [
  { path: 'fixtures/wallet.json', pkg: 'host', name: 'wallet.json', source: walletFixture, note: 'state, credentials, what a query answers' },
]

/// A project: one package, the host it talks to, and the publisher's own server.
///
/// The wallet is deliberately **not** per project. A person has one wallet, and
/// every application they install looks at the same one — which is the whole
/// shape of this platform, and worth showing rather than saying. The handler is
/// per project, because a publisher's server and issuer key are theirs alone.
export type Project = {
  id: string
  name: string
  note: string
  server: SourceFile
}

const handler = (id: string): SourceFile => ({
  path: `server/${id}/handler.ts`,
  pkg: 'server',
  name: 'handler.ts',
  source: DEFAULT_HANDLER,
  note: 'verify the record, then issue or refuse',
})

export const projects: Project[] = [
  {
    id: 'loyalty',
    name: 'Loyalty card',
    note: 'every phase, a screen, and a credential issued at the end',
    server: handler('loyalty'),
  },
  {
    id: 'portfolio',
    name: 'Portfolio',
    note: 'no state, no issuance, a proof that discloses nothing',
    server: handler('portfolio'),
  },
  {
    id: 'door',
    name: 'Door',
    note: 'prove an age, disclose nothing else',
    server: handler('door'),
  },
  {
    id: 'rejected',
    name: 'Refused programs',
    note: 'twelve programs and the error each one is owed',
    server: handler('rejected'),
  },
]

export const docs = [
  { path: 'docs/spec.md', name: 'The language', source: spec },
  { path: 'README.md', name: 'Why', source: readme },
  { path: 'examples/README.md', name: 'Examples', source: examplesReadme },
]

export const text: Record<string, Record<string, string>> = JSON.parse(textBundle).keys
