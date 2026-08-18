import loyalty from '../../examples/loyalty.val?raw'
import portfolio from '../../examples/portfolio.val?raw'
import wallet from '../../examples/wallet.val?raw'
import door from '../../examples/door.val?raw'
import textBundle from '../../examples/text.json?raw'
import portfolioText from '../../examples/portfolio-text.json?raw'
import referendum from '../../examples/referendum.val?raw'
import referendumText from '../../examples/referendum-text.json?raw'
import referendumHandler from '../../examples/referendum-handler.rs?raw'
import transit from '../../examples/transit.val?raw'
import transitText from '../../examples/transit-text.json?raw'
import transitHandler from '../../examples/transit-handler.go?raw'
import catalogue from '../../examples/catalogue.val?raw'
import catalogueText from '../../examples/catalogue-text.json?raw'
import condo from '../../examples/condo.val?raw'
import condoText from '../../examples/condo-text.json?raw'
import condoHandler from '../../examples/condo-handler.py?raw'
import walletFixture from '../../fixtures/wallet.json?raw'

/// The one wallet. Every project previews against the same phone, because a
/// person has one.
export const HOST = 'fixtures/wallet.json'
import { DEFAULT_HANDLER, serverStarter, STARTER_HANDLER } from './server'

import spec from '../../docs/spec.md?raw'
import readme from '../../README.md?raw'
import examplesReadme from '../../examples/README.md?raw'

/// `pkg` is which package a file belongs to. A package is several files sharing
/// one scope, so the playground analyses them together — `wallet.val` presses an
/// action `loyalty.val` declares, and either alone is half a program.
///
/// `added` marks a file somebody made here rather than one that shipped with
/// the playground. The examples are what the documentation points at, so they
/// can be added to and read but not taken apart.
export type SourceFile = {
  path: string
  name: string
  source: string
  note: string
  pkg: string
  added?: boolean
}

// Read out of the repository rather than copied into the playground. A copy is
// how a playground starts teaching a language that no longer exists.
export const files: SourceFile[] = [
  { path: 'examples/loyalty.val', pkg: 'loyalty', name: 'loyalty.val', source: loyalty, note: 'every phase, both verifiability types' },
  { path: 'examples/portfolio.val', pkg: 'portfolio', name: 'portfolio.val', source: portfolio, note: 'no state, no issuance, a proof that discloses nothing' },
  { path: 'examples/wallet.val', pkg: 'loyalty', name: 'wallet.val', source: wallet, note: 'a screen — the second file of the loyalty package' },
  { path: 'examples/door.val', pkg: 'door', name: 'door.val', source: door, note: 'prove an age, disclose nothing' },
  { path: 'examples/text.json', pkg: 'loyalty', name: 'text.json', source: textBundle, note: 'the signed text bundle' },
  { path: 'examples/portfolio-text.json', pkg: 'portfolio', name: 'text.json', source: portfolioText, note: 'the signed text bundle' },
  { path: 'examples/referendum.val', pkg: 'referendum', name: 'referendum.val', source: referendum, note: 'prove you may vote, disclose nothing else' },
  { path: 'examples/referendum-text.json', pkg: 'referendum', name: 'text.json', source: referendumText, note: 'the signed text bundle' },
  { path: 'examples/transit.val', pkg: 'transit', name: 'transit.val', source: transit, note: 'a pass that stays in the wallet' },
  { path: 'examples/transit-text.json', pkg: 'transit', name: 'text.json', source: transitText, note: 'the signed text bundle' },
  { path: 'examples/catalogue.val', pkg: 'catalogue', name: 'catalogue.val', source: catalogue, note: 'every component the wallet ships' },
  { path: 'examples/catalogue-text.json', pkg: 'catalogue', name: 'text.json', source: catalogueText, note: 'the signed text bundle' },
  { path: 'examples/condo.val', pkg: 'condo', name: 'condo.val', source: condo, note: 'a vote weighted by a share of the building' },
  { path: 'examples/condo-text.json', pkg: 'condo', name: 'text.json', source: condoText, note: 'the signed text bundle' },
]

/// Not part of any package: a `.va` never carries somebody's wallet. It is the
/// host's answers — what a screen's declaration resolves to — and it is here so
/// that editing it changes what the preview shows and what a run computes.
export const hostFiles: SourceFile[] = [
  { path: HOST, pkg: 'host', name: 'wallet.json', source: walletFixture, note: 'state, credentials, what a query answers' },
]

/// A project: one package, the wallet it looks at, and the publisher's own
/// server. One Micro App, in other words — its own screens, its own preview,
/// its own handler.
///
/// The wallet is deliberately **not** part of one. A person has one, and every
/// application they install looks at that one, which is the shape of this whole
/// platform and is better shown than said.
export type Project = {
  id: string
  name: string
  note: string
  files: SourceFile[]
  servers: SourceFile[]
  /// Shipped with the editor, and not removable. Somebody else's project is
  /// theirs to delete.
  builtin: boolean
}

const handler = (id: string, source = DEFAULT_HANDLER, name = 'handler.ts'): SourceFile => ({
  path: `server/${id}/${name}`,
  pkg: 'server',
  name,
  source,
  note: 'verify the record, then issue or refuse',
})

const example = (
  id: string,
  name: string,
  note: string,
  files: SourceFile[],
  server?: { name: string; source: string },
): Project => ({
  id,
  name,
  note,
  files,
  servers: [server ? handler(id, server.source, server.name) : handler(id)],
  builtin: true,
})

export const examples: Project[] = [
  example('loyalty', 'Loyalty card', 'every phase, a screen, and a credential issued at the end',
    files.filter((f) => f.pkg === 'loyalty')),
  example('portfolio', 'Portfolio', 'no state, no issuance, a proof that discloses nothing',
    files.filter((f) => f.pkg === 'portfolio')),
  example('door', 'Door', 'prove an age, disclose nothing else',
    files.filter((f) => f.pkg === 'door')),
  example('referendum', 'Referendum', 'one ballot per voter, counted without a list of who voted',
    files.filter((f) => f.pkg === 'referendum'),
    { name: 'handler.rs', source: referendumHandler }),
  example('catalogue', 'Component catalogue', 'every component on one screen, and what each prop does',
    files.filter((f) => f.pkg === 'catalogue')),
  example('condo', 'Condominium meeting', 'a vote weighted by ownership, with the statutory cap checked on the server',
    files.filter((f) => f.pkg === 'condo'),
    { name: 'handler.py', source: condoHandler }),
  example('transit', 'Transit pass', 'tap to ride, with the fare cap on the operator\'s server',
    files.filter((f) => f.pkg === 'transit'),
    { name: 'handler.go', source: transitHandler }),
]

/// What a new project starts as: the smallest thing that compiles, runs, and
/// leaves a record. No credential, because the wallet is somebody else's file
/// and a starter that failed on the first press would teach the wrong lesson —
/// reading one is the next page of the guide, not the first.
const STARTER_APP = `app "example.new"
version 1

capabilities {
}

state {
  taps: int default 0
}

action Tap {
  compute {
    const next = state.taps + 1
  }

  update {
    taps: next
  }
}

screen Home {
  column {
    card(text: "count", taps: state.taps)
    button(text: "tap", emphasis: primary, onTap: Tap)
  }
}
`

const STARTER_TEXT = `{
  "locales": ["en", "th"],
  "keys": {
    "count": { "en": "Tapped {taps} times", "th": "กดไปแล้ว {taps} ครั้ง" },
    "tap":   { "en": "Tap", "th": "กด" }
  }
}
`

export function newProject(id: string, name: string): Project {
  return {
    id,
    name,
    note: 'yours — edit the code, press the button, read the log',
    builtin: false,
    files: [
      { path: `${id}/app.val`, pkg: id, name: 'app.val', source: STARTER_APP, note: 'the application' },
      { path: `${id}/text.json`, pkg: id, name: 'text.json', source: STARTER_TEXT, note: 'every sentence a person reads' },
    ],
    servers: [handler(id, STARTER_HANDLER)],
  }
}

/// The text bundle a project ships, read out of its own `text.json`.
///
/// Not a global: every package carries its own, they are signed together, and a
/// shared one would be a sentence somebody else's application is responsible
/// for.
export function bundleOf(files: SourceFile[], sources: Record<string, string>) {
  const file = files.find((f) => f.name === 'text.json')
  if (!file) return { keys: {}, locales: ['en', 'th'] }
  try {
    const parsed = JSON.parse(sources[file.path] ?? file.source)
    return {
      keys: (parsed.keys ?? {}) as Record<string, Record<string, string>>,
      locales: (parsed.locales ?? ['en', 'th']) as string[],
    }
  } catch {
    // A bundle being edited is a bundle that is briefly not JSON. Reporting it
    // as missing every key would bury the one problem that is real.
    return { keys: {}, locales: ['en', 'th'] }
  }
}

/// The three places a file can be: the package that is signed and installed,
/// the host's own data, and the publisher's server. They are not
/// interchangeable, which is the point of showing them apart.
export type Group = 'package' | 'host' | 'server'

/// The file a group starts as when somebody adds one.
///
/// Enough to compile or to run, because an empty file reports an error before
/// it has been typed into, and the first thing a person then does is delete it.
export function blankFile(group: Group, id: string, name: string): SourceFile {
  if (group === 'host') {
    return {
      path: `fixtures/${name}`,
      pkg: 'host',
      name,
      source: '{\n}\n',
      note: 'more of what the host answers',
      added: true,
    }
  }
  if (group === 'server') {
    return {
      path: `server/${id}/${name}`,
      pkg: 'server',
      name,
      source: serverStarter(name),
      note: name.startsWith('handler.') ? 'verify the record, then issue or refuse' : 'a module beside the handler',
      added: true,
    }
  }
  return {
    path: `${id}/${name}`,
    pkg: id,
    name,
    source: name.endsWith('.json')
      ? '{\n  "locales": ["en", "th"],\n  "keys": {}\n}\n'
      : `// The rest of the package. One scope, so this file sees what the others\n// declare and declares for them in turn.\n`,
    note: 'part of the package',
    added: true,
  }
}

/// What a group will accept. A `.val` in the host group would be analysed by
/// nothing and a `.json` in the server group would be run by nothing; both look
/// like a file that quietly does not work.
export const ALLOWED: Record<Group, string[]> = {
  package: ['.val', '.json'],
  host: ['.json'],
  server: ['.ts', '.go', '.rs', '.py'],
}
