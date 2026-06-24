// Copy + figures for the single-screen landing page. Figures are taken
// verbatim from the Skarn README.

export const REPO = 'https://github.com/Rani367/Skarn'
export const SITE = 'https://rani367.github.io/Skarn/'
export const CRATE = 'https://crates.io/crates/skarn'
export const INSTALL_ONELINER = 'curl -fsSL https://rani367.github.io/Skarn/install.sh | sh'
export const CARGO_INSTALL = 'cargo install skarn'

export const TAGLINE =
  'A fast, OS-sandboxed Model Context Protocol gateway with an embedded Code Mode engine and shell-output token compression — in a single Rust binary.'

export const PUNCHLINE =
  "Cut your agent's API bill while physically stopping it from wiping your disk or exfiltrating your secrets."

/* the attention-grabbing numbers, promoted into the header --------------- */
export const HEADER_STATS = [
  { value: 99, prefix: '', suffix: '%', label: 'fewer tokens' },
  { value: 90, prefix: '', suffix: '%', label: 'smaller logs' },
  { value: 5, prefix: '<', suffix: 'ms', label: 'sandbox start' },
] as const

/* the three pillars, distilled to one line each -------------------------- */
export const PILLARS = [
  {
    name: 'Code Mode',
    body: 'An API, not a schema dump — huge intermediate payloads never reach the context window.',
  },
  {
    name: 'Compression',
    body: 'Per-tool filters cut shell-output noise; errors and warnings always survive.',
  },
  {
    name: 'OS sandbox',
    body: 'Kernel-enforced: Seatbelt · Landlock+seccomp · AppContainer. No Docker.',
  },
] as const
