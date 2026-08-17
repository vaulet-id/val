import loyalty from '../../examples/loyalty.val?raw'
import portfolio from '../../examples/portfolio.val?raw'
import wallet from '../../examples/wallet.val?raw'
import door from '../../examples/door.val?raw'
import rejected from '../../examples/rejected.val?raw'
import textBundle from '../../examples/text.json?raw'

import spec from '../../docs/spec.md?raw'
import readme from '../../README.md?raw'
import examplesReadme from '../../examples/README.md?raw'

export type SourceFile = { path: string; name: string; source: string; note: string }

// Read out of the repository rather than copied into the playground. A copy is
// how a playground starts teaching a language that no longer exists.
export const files: SourceFile[] = [
  { path: 'examples/loyalty.val', name: 'loyalty.val', source: loyalty, note: 'every phase, both verifiability types' },
  { path: 'examples/portfolio.val', name: 'portfolio.val', source: portfolio, note: 'no state, no issuance, a proof that discloses nothing' },
  { path: 'examples/wallet.val', name: 'wallet.val', source: wallet, note: 'a screen — declared data, a press that names an action' },
  { path: 'examples/door.val', name: 'door.val', source: door, note: 'prove an age, disclose nothing' },
  { path: 'examples/rejected.val', name: 'rejected.val', source: rejected, note: 'programs that must not compile' },
  { path: 'examples/text.json', name: 'text.json', source: textBundle, note: 'the signed text bundle' },
]

export const docs = [
  { path: 'docs/spec.md', name: 'The language', source: spec },
  { path: 'README.md', name: 'Why', source: readme },
  { path: 'examples/README.md', name: 'Examples', source: examplesReadme },
]

export const text: Record<string, Record<string, string>> = JSON.parse(textBundle).keys
