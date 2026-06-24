import { motion, useReducedMotion } from 'framer-motion'
import { CARGO_INSTALL, CRATE, INSTALL_ONELINER, PILLARS, PUNCHLINE, REPO, TAGLINE } from '../data/content'
import { TurbulentFlow } from './ui/turbulent-flow'
import { CopyButton } from './ui/CopyButton'
import { GitHubIcon, RustIcon, ShieldIcon } from './ui/Icons'
import './hero.css'

export function Hero() {
  const reduce = useReducedMotion()

  const container = {
    hidden: {},
    show: { transition: { staggerChildren: reduce ? 0 : 0.08, delayChildren: 0.15 } },
  }
  const item = {
    hidden: reduce ? { opacity: 0 } : { opacity: 0, y: 20 },
    show: { opacity: 1, y: 0, transition: { duration: 0.8, ease: [0.16, 1, 0.3, 1] } },
  }

  return (
    <section className="hero" id="top">
      <TurbulentFlow className="hero-flow" />
      <div className="hero-scrim" aria-hidden="true" />
      <div className="hero-grain" aria-hidden="true" />

      <motion.div className="hero-content container" variants={container} initial="hidden" animate="show">
        <motion.h1 className="hero-title" variants={item}>
          Sandbox your agent.
          <br />
          Shrink its token bill.
        </motion.h1>

        <motion.p className="hero-tagline" variants={item}>
          {TAGLINE}
        </motion.p>

        <motion.p className="hero-punch" variants={item}>
          {PUNCHLINE}
        </motion.p>

        <motion.div className="hero-install" variants={item}>
          <span className="install-prompt mono" aria-hidden="true">
            $
          </span>
          <code className="install-cmd mono">{INSTALL_ONELINER}</code>
          <CopyButton text={INSTALL_ONELINER} />
        </motion.div>

        <motion.div className="hero-actions" variants={item}>
          <a className="btn btn-primary" href={REPO} target="_blank" rel="noreferrer noopener">
            <GitHubIcon width={18} height={18} /> Star on GitHub
          </a>
          <a className="hero-crate mono" href={CRATE} target="_blank" rel="noreferrer noopener">
            <RustIcon width={16} height={16} /> {CARGO_INSTALL}
          </a>
          <a className="hero-security mono" href={`${REPO}/blob/main/SECURITY.md`} target="_blank" rel="noreferrer noopener">
            <ShieldIcon width={15} height={15} /> threat model
          </a>
        </motion.div>

        <motion.ul className="hero-pillars" variants={item}>
          {PILLARS.map((p) => (
            <li className="pillar" key={p.name}>
              <span className="pillar-name mono">{p.name}</span>
              <p className="pillar-body">{p.body}</p>
            </li>
          ))}
        </motion.ul>
      </motion.div>
    </section>
  )
}
