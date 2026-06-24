import { HEADER_STATS, REPO } from '../data/content'
import { Counter } from './ui/Counter'
import { GitHubIcon, ShieldIcon } from './ui/Icons'
import './nav.css'

export function Nav() {
  return (
    <header className="nav">
      <div className="nav-inner container">
        <a className="nav-brand" href="#top" aria-label="Skarn — home">
          <span className="nav-mark">
            <ShieldIcon width={18} height={18} />
          </span>
          <span className="nav-word">Skarn</span>
        </a>

        <dl className="nav-stats" aria-label="Headline numbers">
          {HEADER_STATS.map((s) => (
            <div className="nav-stat" key={s.label}>
              <dd className="nav-stat-num">
                <Counter to={s.value} prefix={s.prefix} suffix={s.suffix} duration={1.6} />
              </dd>
              <dt className="nav-stat-label">{s.label}</dt>
            </div>
          ))}
        </dl>

        <a className="nav-gh" href={REPO} target="_blank" rel="noreferrer noopener">
          <GitHubIcon width={18} height={18} />
          <span>GitHub</span>
        </a>
      </div>
    </header>
  )
}
