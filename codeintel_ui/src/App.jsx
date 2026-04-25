import { useState, useRef, useEffect, Component } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import ThreeBackground from './components/ThreeBackground'
import DependencyGraph  from './components/DependencyGraph'
import { useAnalyzer, useChat } from './hooks/useAnalyzer'

// ─── Error Boundary — prevents full UI crash on graph failure ─────────────────
class GraphErrorBoundary extends Component {
  constructor(props) { super(props); this.state = { hasError: false, error: null } }
  static getDerivedStateFromError(error) { return { hasError: true, error } }
  componentDidCatch(error, info) { console.error('Graph error:', error, info) }
  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
          height: '100%', gap: 10, color: 'var(--text-secondary)',
        }}>
          <div style={{ fontSize: 28, opacity: 0.3 }}>⬡</div>
          <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>Graph render error</div>
          <div style={{ fontSize: 11, color: 'var(--text-dim)', maxWidth: 300, textAlign: 'center' }}>
            {this.state.error?.message || 'Unknown error'}
          </div>
          <button
            onClick={() => this.setState({ hasError: false, error: null })}
            style={{
              marginTop: 8, background: 'var(--accent-dim)', border: '1px solid var(--accent)',
              borderRadius: 'var(--radius-sm)', color: 'var(--accent)', padding: '5px 14px',
              cursor: 'pointer', fontSize: 11, fontFamily: 'var(--font-mono)',
            }}
          >Retry</button>
        </div>
      )
    }
    return this.props.children
  }
}

// ─── Language badge colors ────────────────────────────────────────────────────
const LANG_BADGE = {
  Python:     { bg: '#3b82f620', color: '#60a5fa', label: 'PY' },
  JavaScript: { bg: '#f59e0b20', color: '#fbbf24', label: 'JS' },
  TypeScript: { bg: '#6366f120', color: '#a5b4fc', label: 'TS' },
  Java:       { bg: '#ef444420', color: '#f87171', label: 'JV' },
  'C++':      { bg: '#a855f720', color: '#c084fc', label: 'C++'},
  C:          { bg: '#ec489920', color: '#f472b6', label: 'C'  },
  Go:         { bg: '#06b6d420', color: '#22d3ee', label: 'GO' },
  default:    { bg: '#6b728020', color: '#9ca3af', label: '?'  },
}

const SUGGESTED = [
  'What are the highest-risk files?',
  'Explain any circular dependencies',
  'Which file is most critical?',
  'What errors were detected?',
  'Which files are entry points?',
]

const TABS = [
  { id: 'overview',   icon: '◈', label: 'Overview'   },
  { id: 'graph',      icon: '⬡', label: 'Graph'       },
  { id: 'issues',     icon: '⚑', label: 'Issues'      },
  { id: 'complexity', icon: '◎', label: 'Complexity'  },
  { id: 'chat',       icon: '✦', label: 'AI Chat'     },
]

// ─── Helpers ──────────────────────────────────────────────────────────────────
const fmtBytes = b => {
  if (!b) return '0 B'
  const u = ['B','KB','MB']; let i = 0
  while (b >= 1024 && i < 2) { b /= 1024; i++ }
  return `${b.toFixed(1)} ${u[i]}`
}

// ─── Sub-components ───────────────────────────────────────────────────────────

function StatCard({ label, value, sub, color = 'default', delay = 0 }) {
  return (
    <motion.div
      className="stat-card"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay, duration: 0.4, ease: [0.4, 0, 0.2, 1] }}
    >
      <div className="stat-label">{label}</div>
      <div className={`stat-value ${color}`}>{value}</div>
      {sub && <div className="stat-sub">{sub}</div>}
    </motion.div>
  )
}

function LangBar({ lang, pct, color }) {
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 5 }}>
        <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{lang}</span>
        <span style={{ fontSize: 12, color, fontFamily: 'var(--font-mono)' }}>{pct.toFixed(1)}%</span>
      </div>
      <div className="progress-bar">
        <motion.div
          className="progress-fill"
          style={{ background: color }}
          initial={{ width: 0 }}
          animate={{ width: `${pct}%` }}
          transition={{ duration: 1, ease: [0.4, 0, 0.2, 1], delay: 0.2 }}
        />
      </div>
    </div>
  )
}

function IssueItem({ issue, idx }) {
  const icons = {
    syntax_error:          '⊘',
    circular_dependency:   '↺',
    isolated_file:         '◌',
    excessive_dependencies:'⚡',
    unused_function:       '∅',
    missing_import:        '?',
  }
  const sevMap  = { high: 'ERROR', medium: 'WARN', low: 'INFO' }
  const colorMap = { high: 'var(--red)', medium: 'var(--yellow)', low: 'var(--text-dim)' }

  return (
    <motion.div
      className={`issue-item ${issue.severity}`}
      initial={{ opacity: 0, x: -12 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: idx * 0.03, duration: 0.3 }}
      style={{
        borderLeft: `3px solid ${colorMap[issue.severity]}`,
        background: issue.severity === 'high' ? 'var(--red-dim)' :
                    issue.severity === 'medium' ? 'var(--yellow-dim)' : 'var(--bg-raised)',
        borderColor: 'transparent',
        borderLeftColor: colorMap[issue.severity],
      }}
    >
      <span className="issue-icon" style={{ color: colorMap[issue.severity] }}>
        {icons[issue.issue_type] || '•'}
      </span>
      <div className="issue-body">
        <div className="issue-file">{issue.file}</div>
        <div className="issue-msg">{issue.message}</div>
      </div>
      <span
        className={`issue-badge badge-${issue.severity}`}
        style={{
          background: issue.severity === 'high' ? 'var(--red-dim)' :
                      issue.severity === 'medium' ? 'var(--yellow-dim)' : 'var(--bg-overlay)',
          color: colorMap[issue.severity],
          border: `1px solid ${colorMap[issue.severity]}44`,
        }}
      >
        {sevMap[issue.severity]}
      </span>
    </motion.div>
  )
}

function LoadingOverlay({ stages, currentStage }) {
  return (
    <div className="loading-overlay">
      <div className="loading-ring" />
      <div className="loading-text">Analyzing project…</div>
      <div className="loading-stages">
        {stages.map((s, i) => (
          <div
            key={s.id}
            className={`stage-row ${i < currentStage ? 'done' : i === currentStage ? 'active' : ''}`}
          >
            <span className="stage-check">
              {i < currentStage ? '✓' : i === currentStage ? '›' : '·'}
            </span>
            <span>{s.label}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

// ─── Chat message bubble ──────────────────────────────────────────────────────
function ChatBubble({ msg }) {
  const confStyle = !msg.confidence ? null :
    msg.confidence >= 80 ? 'confidence-high' :
    msg.confidence >= 50 ? 'confidence-medium' : 'confidence-low'

  if (msg.role === 'error') {
    return (
      <div className="chat-bubble system">
        <div className="bubble-content" style={{ borderLeft: '3px solid var(--red)', color: 'var(--red)' }}>
          ⊘ {msg.content}
        </div>
      </div>
    )
  }

  return (
    <motion.div
      className={`chat-bubble ${msg.role}`}
      initial={{ opacity: 0, scale: 0.95, y: 8 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      transition={{ duration: 0.25 }}
    >
      <div className="bubble-content">
        {msg.content}
        {msg.streaming && <span style={{ color: 'var(--cyan)', animation: 'fadeInOut 0.8s infinite' }}>▌</span>}
      </div>
      <div className="bubble-meta">
        <span>{msg.ts}</span>
        {msg.role === 'ai' && msg.confidence != null && (
          <span className={`confidence-badge ${confStyle}`}>
            {msg.confidence}% confidence
          </span>
        )}
      </div>
    </motion.div>
  )
}

// ─── Main App ─────────────────────────────────────────────────────────────────
export default function App() {
  const [path,       setPath]       = useState('')
  const [tab,        setTab]        = useState('overview')
  const [selFile,    setSelFile]    = useState(null)
  const [search,     setSearch]     = useState('')
  const [severityF,  setSeverityF]  = useState('all')
  const [safeExec,   setSafeExec]   = useState(false)
  const chatEndRef = useRef(null)

  const { data, loading, error, stage, stages, analyze } = useAnalyzer()
  const { messages, thinking, sendMessage, clearChat } = useChat(data)
  const [chatInput, setChatInput] = useState('')
  const [chatContext, setChatContext] = useState(null)

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Build graph data — file nodes only (folder nodes built inside DependencyGraph from folder_tree)
  // folder_tree is the list of folder paths from the API
  const graphNodes = (data?.files || []).filter(f => f && f.rel_path).map(f => ({
    id:              f.rel_path?.replace(/\\/g, '/') || '',
    language:        f.language,
    risk:            f.risk || f.complexity?.level || 'low',
    // Fix: use complexity.lines which is now guaranteed non-zero from server fix
    lines:           f.complexity?.lines || 0,
    complexity_level:f.complexity?.level,
    isEntry:         data?.summary?.entry_points?.includes(f.rel_path),
    isCircular:      (data?.issues || []).some(
      i => i.issue_type === 'circular_dependency' && i.file === f.rel_path
    ),
  }))

  const graphEdges = (data?.dependencies || []).map(d => ({
    source: d.source?.replace(/\\/g, '/') || d.source,
    target: d.target?.replace(/\\/g, '/') || d.target,
  }))

  // Language distribution with colors
  const LANG_COLORS_MAP = {
    Python: '#3b82f6', JavaScript: '#f59e0b', TypeScript: '#6366f1',
    Java: '#ef4444', 'C++': '#a855f7', C: '#ec4899', Go: '#06b6d4',
  }
  const langDist = Object.entries(data?.summary?.language_distribution || {})
    .sort((a, b) => b[1] - a[1])

  // Filtered sidebar files
  const filteredFiles = (data?.files || []).filter(f =>
    !search || (f.rel_path && f.rel_path.toLowerCase().includes(search.toLowerCase()))
  )

  // Filtered issues
  const filteredIssues = (data?.issues || []).filter(i =>
    severityF === 'all' || i.severity === severityF
  )

  const handleSend = () => {
    if (!chatInput.trim() || thinking) return
    sendMessage(chatInput, chatContext)
    setChatInput('')
  }

  const handleFileClick = (f) => {
    setSelFile(f)
    setChatContext(f.rel_path)
  }

  const summary = data?.summary || {}

  return (
    <div className="app-shell">
      {/* ── 3D Background ── */}
      <ThreeBackground />

      <div className="app-content">
        {/* ── Top Nav ────────────────────────────────────── */}
        <nav className="topnav">
          <div className="nav-logo">
            <div className="nav-logo-icon">⬡</div>
            <div>
              <div className="nav-logo-text">CodeIntel</div>
              <div className="nav-logo-sub">AI Analyzer</div>
            </div>
          </div>

          <form
            className="nav-path-form"
            onSubmit={e => { e.preventDefault(); analyze(path, { safeExec }) }}
          >
            <input
              className="nav-input"
              value={path}
              onChange={e => setPath(e.target.value)}
              placeholder="Enter project folder path…  e.g. /home/user/my-project"
            />
            <button className="btn-analyze" disabled={loading}>
              {loading ? '⟳ Analyzing…' : '▶ Analyze'}
            </button>
          </form>

            {/* API key is now server-side only (NVIDIA_API_KEY env var) */}
            <div className="nav-status">
              {/* Safe exec toggle */}
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer', fontSize: 11, color: 'var(--text-secondary)' }}>
                <input
                  type="checkbox"
                  checked={safeExec}
                  onChange={e => setSafeExec(e.target.checked)}
                  style={{ accentColor: 'var(--accent)' }}
                />
                Safe Exec
              </label>

              {/* Status */}
              <div
                className={`status-dot ${loading ? 'busy' : error ? 'err' : data ? 'ok' : ''}`}
              />
              <span className="status-text">
                {loading ? 'Analyzing…' : error ? error.slice(0, 40) : data ? `${summary.total_files} files · ${summary.total_issues} issues` : 'Ready'}
              </span>
            </div>
        </nav>

        {/* ── Main Layout ─────────────────────────────────── */}
        <div className="main-layout">
          {/* ── Sidebar ─────────────────────────────────── */}
          <aside className="sidebar">
            <div className="sidebar-header">
              <span className="sidebar-title">Files</span>
              <span className="sidebar-count">{filteredFiles.length}</span>
            </div>
            <div className="sidebar-search">
              <input
                value={search}
                onChange={e => setSearch(e.target.value)}
                placeholder="Search files…"
              />
            </div>
            <div className="file-list">
              {filteredFiles.length === 0 && (
                <div style={{ padding: '24px 16px', color: 'var(--text-dim)', fontSize: 11, textAlign: 'center' }}>
                  {data ? 'No files match' : 'Analyze a project to see files'}
                </div>
              )}
              {filteredFiles.map(f => {
                const badge = LANG_BADGE[f.language] || LANG_BADGE.default
                const risk  = f.complexity?.level || 'none'
                return (
                  <div
                    key={f.rel_path}
                    className={`file-item ${selFile?.rel_path === f.rel_path ? 'selected' : ''}`}
                    onClick={() => handleFileClick(f)}
                  >
                    <div className={`risk-dot ${risk}`} />
                    <span className="file-name" title={f.rel_path}>
                      {(f.rel_path || '').replace(/\\/g, '/').split('/').pop() || f.name || '?'}
                    </span>
                    <span
                      className="file-lang-badge"
                      style={{ background: badge.bg, color: badge.color }}
                    >
                      {badge.label}
                    </span>
                  </div>
                )
              })}
            </div>

            {/* Sidebar file detail */}
            {selFile && (
              <div style={{
                borderTop: '1px solid var(--border-dim)',
                padding: '12px 14px',
                background: 'var(--bg-surface)',
                fontSize: 11,
              }}>
                <div style={{ color: 'var(--text-primary)', fontWeight: 600, marginBottom: 6, wordBreak: 'break-all' }}>
                  {(selFile.rel_path || '').replace(/\\/g, '/').split('/').pop() || selFile.name || '?'}
                </div>
                <div style={{ color: 'var(--text-dim)', marginBottom: 2 }}>{selFile.language} · {fmtBytes(selFile.size)}</div>
                {selFile.complexity && (
                  <div style={{ color: 'var(--text-dim)' }}>
                    {selFile.complexity.lines}L ·{' '}
                    <span style={{
                      color: selFile.complexity.level === 'high' ? 'var(--red)' :
                             selFile.complexity.level === 'medium' ? 'var(--yellow)' : 'var(--green)'
                    }}>
                      {selFile.complexity.level} complexity
                    </span>
                  </div>
                )}
              </div>
            )}
          </aside>

          {/* ── Main Content ─────────────────────────────── */}
          <main className="main-panel">
            {/* Tab bar */}
            <div className="tab-bar">
              {TABS.map(t => {
                const badge = t.id === 'issues' && data ? data.issues?.length :
                              t.id === 'overview' && data ? data.files?.length : null
                return (
                  <button
                    key={t.id}
                    className={`tab-btn ${tab === t.id ? 'active' : ''}`}
                    onClick={() => setTab(t.id)}
                  >
                    <span>{t.icon}</span>
                    <span>{t.label}</span>
                    {badge != null && <span className="tab-badge">{badge}</span>}
                  </button>
                )
              })}
            </div>

            {/* Tab panels */}
            <div className="tab-content">
              <AnimatePresence mode="wait">
                <motion.div
                  key={tab}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -8 }}
                  transition={{ duration: 0.2 }}
                  style={{ height: '100%', overflow: 'hidden' }}
                >

                  {/* ── OVERVIEW ─────────────────────────── */}
                  {tab === 'overview' && (
                    <div className="scroll-area">
                      {!data && !loading && (
                        <div className="empty-state">
                          <div className="empty-icon">⬡</div>
                          <div className="empty-title">No project analyzed yet</div>
                          <div className="empty-sub">Enter a folder path in the top bar and click Analyze to begin.</div>
                        </div>
                      )}

                      {error && (
                        <div style={{
                          margin: '20px 0', padding: '14px 18px',
                          background: 'var(--red-dim)', border: '1px solid var(--red)',
                          borderRadius: 'var(--radius-md)', color: 'var(--red)', fontSize: 12,
                        }}>
                          ⊘ {error}
                        </div>
                      )}

                      {data && (
                        <>
                          {/* Stat grid */}
                          <div className="stat-grid" style={{ marginBottom: 20 }}>
                            <StatCard label="Total Files"    value={summary.total_files || 0}  color="accent" delay={0} />
                            <StatCard label="Issues Found"   value={summary.total_issues || 0}
                              color={summary.total_issues > 0 ? 'red' : 'green'} delay={0.05}
                              sub={`${summary.syntax_errors || 0} syntax errors`}
                            />
                            <StatCard label="High Risk"      value={summary.high_risk || 0}    color={summary.high_risk > 0 ? 'red' : 'green'} delay={0.1} />
                            <StatCard label="Circular Deps"  value={summary.circular_dependency_count || 0}
                              color={summary.circular_dependency_count > 0 ? 'yellow' : 'green'} delay={0.15}
                            />
                            <StatCard label="Entry Points"   value={summary.entry_points?.length || 0} color="accent" delay={0.2} />
                            <StatCard label="Dead Files"     value={summary.dead_files?.length || 0} delay={0.25} />
                          </div>

                          {/* Two column layout */}
                          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 16 }}>
                            {/* Language dist */}
                            <div className="card fade-in">
                              <div className="card-title">
                                <div className="card-title-dot" />
                                Language Distribution
                              </div>
                              {langDist.map(([lang, pct]) => (
                                <LangBar
                                  key={lang}
                                  lang={lang}
                                  pct={pct}
                                  color={LANG_COLORS_MAP[lang] || '#6b7280'}
                                />
                              ))}
                            </div>

                            {/* Key findings */}
                            <div className="card fade-in stagger-2">
                              <div className="card-title">
                                <div className="card-title-dot" style={{ background: 'var(--cyan)' }} />
                                Key Findings
                              </div>
                              <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                                {summary.most_central_file && (
                                  <div>
                                    <div style={{ fontSize: 10, color: 'var(--text-dim)', marginBottom: 3, textTransform: 'uppercase', letterSpacing: 1 }}>Most Central</div>
                                    <code style={{ fontSize: 11, color: 'var(--cyan)' }}>{summary.most_central_file}</code>
                                  </div>
                                )}
                                {summary.entry_points?.length > 0 && (
                                  <div>
                                    <div style={{ fontSize: 10, color: 'var(--text-dim)', marginBottom: 3, textTransform: 'uppercase', letterSpacing: 1 }}>Entry Points</div>
                                    {summary.entry_points.slice(0, 3).map(ep => (
                                      <div key={ep} style={{ fontSize: 11, color: 'var(--green)', display: 'flex', alignItems: 'center', gap: 5 }}>
                                        <span>▶</span><code>{ep}</code>
                                      </div>
                                    ))}
                                  </div>
                                )}
                                {summary.high_risk_files?.length > 0 && (
                                  <div>
                                    <div style={{ fontSize: 10, color: 'var(--text-dim)', marginBottom: 3, textTransform: 'uppercase', letterSpacing: 1 }}>High Risk</div>
                                    {summary.high_risk_files.slice(0, 3).map(f => (
                                      <div key={f} style={{ fontSize: 11, color: 'var(--red)', display: 'flex', alignItems: 'center', gap: 5 }}>
                                        <span>▲</span><code>{f}</code>
                                      </div>
                                    ))}
                                  </div>
                                )}
                              </div>
                            </div>
                          </div>

                          {/* Recent issues preview */}
                          {data.issues?.length > 0 && (
                            <div className="card fade-in stagger-3">
                              <div className="card-title">
                                <div className="card-title-dot" style={{ background: 'var(--red)' }} />
                                Recent Issues
                                <span style={{ marginLeft: 'auto', fontSize: 10, color: 'var(--text-dim)', fontWeight: 400 }}>
                                  Showing {Math.min(5, data.issues.length)} of {data.issues.length}
                                </span>
                              </div>
                              {data.issues.slice(0, 5).map((issue, i) => (
                                <IssueItem key={i} issue={issue} idx={i} />
                              ))}
                              {data.issues.length > 5 && (
                                <button
                                  onClick={() => setTab('issues')}
                                  style={{
                                    marginTop: 8, background: 'none', border: '1px solid var(--border-mid)',
                                    borderRadius: 'var(--radius-sm)', color: 'var(--accent)',
                                    padding: '6px 14px', cursor: 'pointer', fontSize: 11,
                                    fontFamily: 'var(--font-mono)', width: '100%',
                                  }}
                                >
                                  View all {data.issues.length} issues →
                                </button>
                              )}
                            </div>
                          )}
                        </>
                      )}
                    </div>
                  )}

                  {/* ── GRAPH ────────────────────────────── */}
                  {tab === 'graph' && (
                    <>
                      {loading && <LoadingOverlay stages={stages} currentStage={stage} />}
                      {!data && !loading && (
                        <div className="empty-state">
                          <div className="empty-icon">⬡</div>
                          <div className="empty-title">No graph yet</div>
                          <div className="empty-sub">Analyze a project to see the dependency graph.</div>
                        </div>
                      )}
                      {data && (
                        <GraphErrorBoundary>
                          <DependencyGraph
                            nodes={graphNodes}
                            edges={graphEdges}
                            highlightNode={selFile?.rel_path?.replace(/\\/g, '/')}
                            onNodeClick={n => {
                              if (n.type === 'file') {
                                const f = data.files.find(f =>
                                  f.rel_path?.replace(/\\/g, '/') === n.id
                                )
                                if (f) handleFileClick(f)
                              }
                            }}
                          />
                        </GraphErrorBoundary>
                      )}
                    </>
                  )}

                  {/* ── ISSUES ───────────────────────────── */}
                  {tab === 'issues' && (
                    <div className="scroll-area">
                      {!data ? (
                        <div className="empty-state">
                          <div className="empty-icon">⚑</div>
                          <div className="empty-title">No issues yet</div>
                          <div className="empty-sub">Analyze a project to detect issues.</div>
                        </div>
                      ) : (
                        <>
                          {/* Filter bar */}
                          <div style={{ display: 'flex', gap: 8, marginBottom: 16, alignItems: 'center' }}>
                            {['all','high','medium','low'].map(s => (
                              <button
                                key={s}
                                onClick={() => setSeverityF(s)}
                                style={{
                                  padding: '5px 12px',
                                  background: severityF === s ? 'var(--accent-dim)' : 'var(--bg-raised)',
                                  border: `1px solid ${severityF === s ? 'var(--accent)' : 'var(--border-dim)'}`,
                                  borderRadius: 'var(--radius-sm)',
                                  color: severityF === s ? 'var(--accent)' : 'var(--text-dim)',
                                  cursor: 'pointer', fontSize: 11,
                                  fontFamily: 'var(--font-mono)',
                                  textTransform: 'capitalize',
                                }}
                              >
                                {s === 'all' ? `All (${data.issues.length})` :
                                 `${s} (${data.issues.filter(i => i.severity === s).length})`}
                              </button>
                            ))}
                          </div>

                          {filteredIssues.length === 0 && (
                            <div style={{ color: 'var(--green)', fontSize: 13, padding: 20, textAlign: 'center' }}>
                              ✓ No issues match the selected filter
                            </div>
                          )}
                          {filteredIssues.map((issue, i) => (
                            <IssueItem key={i} issue={issue} idx={i} />
                          ))}
                        </>
                      )}
                    </div>
                  )}

                  {/* ── COMPLEXITY ───────────────────────── */}
                  {tab === 'complexity' && (
                    <div className="scroll-area">
                      {!data ? (
                        <div className="empty-state">
                          <div className="empty-icon">◎</div>
                          <div className="empty-title">No complexity data</div>
                          <div className="empty-sub">Analyze a project to see complexity scores.</div>
                        </div>
                      ) : (
                        <>
                          {/* Summary chips */}
                          <div style={{ display: 'flex', gap: 10, marginBottom: 20, flexWrap: 'wrap' }}>
                            {['high','medium','low'].map(lvl => {
                              const count = (data.complexity || []).filter(c => c.level === lvl).length
                              const color = lvl === 'high' ? 'var(--red)' : lvl === 'medium' ? 'var(--yellow)' : 'var(--green)'
                              return (
                                <div key={lvl} style={{
                                  padding: '8px 16px',
                                  background: lvl === 'high' ? 'var(--red-dim)' : lvl === 'medium' ? 'var(--yellow-dim)' : 'var(--green-dim)',
                                  border: `1px solid ${color}44`,
                                  borderRadius: 'var(--radius-md)',
                                  display: 'flex', alignItems: 'center', gap: 8,
                                }}>
                                  <span style={{ color, fontSize: 18, fontWeight: 700 }}>{count}</span>
                                  <span style={{ fontSize: 11, color, textTransform: 'capitalize' }}>{lvl} complexity</span>
                                </div>
                              )
                            })}
                          </div>

                          {/* Bar chart */}
                          <div className="card">
                            <div className="card-title">
                              <div className="card-title-dot" style={{ background: 'var(--accent)' }} />
                              Complexity Scores — All Files
                            </div>
                            {[...(data.complexity || [])]
                              .sort((a, b) => b.score - a.score)
                              .map((c, i) => {
                                const max = Math.max(...data.complexity.map(x => x.score), 1)
                                const pct = (c.score / max) * 100
                                const color = c.level === 'high' ? 'var(--red)' :
                                              c.level === 'medium' ? 'var(--yellow)' : 'var(--green)'
                                return (
                                  <motion.div
                                    key={c.file}
                                    className="complexity-row"
                                    initial={{ opacity: 0 }}
                                    animate={{ opacity: 1 }}
                                    transition={{ delay: i * 0.02 }}
                                  >
                                    <div className="cx-name" title={c.file}>
                                      {c.file.split(/[/\\]/).pop()}
                                    </div>
                                    <div className="cx-bar-wrap">
                                      <motion.div
                                        className="cx-bar"
                                        style={{ background: color }}
                                        initial={{ width: 0 }}
                                        animate={{ width: `${pct}%` }}
                                        transition={{ delay: i * 0.02 + 0.1, duration: 0.6, ease: [0.4,0,0.2,1] }}
                                      />
                                    </div>
                                    <span className="cx-score">{c.score}</span>
                                    <span
                                      className="cx-level"
                                      style={{
                                        background: c.level === 'high' ? 'var(--red-dim)' : c.level === 'medium' ? 'var(--yellow-dim)' : 'var(--green-dim)',
                                        color,
                                      }}
                                    >
                                      {c.level}
                                    </span>
                                  </motion.div>
                                )
                              })}
                          </div>
                        </>
                      )}
                    </div>
                  )}

                  {/* ── CHAT ─────────────────────────────── */}
                  {tab === 'chat' && (
                    <div className="chat-panel">
                      {/* Messages */}
                      <div className="chat-messages">
                        {messages.length === 0 && (
                          <div className="empty-state" style={{ height: 'auto', padding: '30px 20px' }}>
                            <div className="empty-icon" style={{ fontSize: 28 }}>✦</div>
                            <div className="empty-title">AI Code Assistant</div>
                            <div className="empty-sub">
                              Analyze a project, then ask questions about its structure, risks, and issues.
                              Set <code style={{ color: 'var(--cyan)' }}>NVIDIA_API_KEY</code> as a server environment variable to enable AI.
                            </div>
                          </div>
                        )}
                        {messages.map((msg, i) => <ChatBubble key={i} msg={msg} />)}
                        <div ref={chatEndRef} />
                      </div>

                      {/* Context indicator */}
                      {chatContext && (
                        <div style={{
                          padding: '6px 16px',
                          borderTop: '1px solid var(--border-dim)',
                          fontSize: 10,
                          color: 'var(--text-dim)',
                          display: 'flex',
                          alignItems: 'center',
                          gap: 6,
                        }}>
                          <span style={{ color: 'var(--cyan)' }}>◈</span>
                          Context: <code style={{ color: 'var(--cyan)' }}>{chatContext}</code>
                          <button
                            onClick={() => setChatContext(null)}
                            style={{ marginLeft: 'auto', background: 'none', border: 'none', color: 'var(--text-dim)', cursor: 'pointer', fontSize: 12 }}
                          >×</button>
                        </div>
                      )}

                      {/* Suggestions */}
                      {messages.length < 2 && (
                        <div className="chat-suggestions">
                          {SUGGESTED.map(q => (
                            <button
                              key={q}
                              className="suggestion-chip"
                              onClick={() => setChatInput(q)}
                            >
                              {q}
                            </button>
                          ))}
                        </div>
                      )}

                      {/* Input */}
                      <div className="chat-input-area">
                        <textarea
                          className="chat-textarea"
                          value={chatInput}
                          onChange={e => setChatInput(e.target.value)}
                          placeholder={data ? 'Ask about your project…' : 'Analyze a project first…'}
                          disabled={thinking}
                          rows={1}
                          onKeyDown={e => {
                            if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend() }
                          }}
                        />
                        <button
                          className="btn-send"
                          onClick={handleSend}
                          disabled={thinking || !chatInput.trim()}
                        >
                          {thinking ? '…' : '↑'}
                        </button>
                      </div>

                      {/* Thinking indicator */}
                      {thinking && (
                        <div style={{ padding: '4px 16px 8px', fontSize: 10, color: 'var(--accent)', display: 'flex', gap: 4, alignItems: 'center' }}>
                          <span style={{ animation: 'fadeInOut 0.6s infinite' }}>●</span>
                          <span style={{ animation: 'fadeInOut 0.6s 0.2s infinite' }}>●</span>
                          <span style={{ animation: 'fadeInOut 0.6s 0.4s infinite' }}>●</span>
                          <span style={{ marginLeft: 4 }}>AI is thinking…</span>
                        </div>
                      )}
                    </div>
                  )}

                </motion.div>
              </AnimatePresence>

              {/* Loading overlay on top of graph/overview */}
              {loading && tab !== 'graph' && (
                <LoadingOverlay stages={stages} currentStage={stage} />
              )}
            </div>
          </main>
        </div>
      </div>
    </div>
  )
}
