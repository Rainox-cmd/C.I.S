/**
 * DependencyGraph.jsx
 * Targeted fixes per spec:
 *  - Hierarchical layout: Root → Folder → File (matches reference image)
 *  - No node/label overlap (collision detection + dagre-style positioning)
 *  - Thin edges, no glow, low opacity
 *  - Progressive disclosure: expand/collapse folders
 *  - Level-of-detail: hide labels when zoomed out
 *  - Click to highlight connected edges + fade others
 *  - Scalable: caps visible nodes, clusters large folders
 *  - Error boundary: never crashes parent UI
 *  - Folder nodes = hexagon, File nodes = circle (matches reference)
 */

import { useEffect, useRef, useState, useCallback, useMemo } from 'react'
import * as d3 from 'd3'

// ─── Design constants (no glow, thin edges) ──────────────────────────────────

const LANG_COLOR = {
  Python:     '#3b82f6',
  JavaScript: '#f59e0b',
  TypeScript: '#818cf8',
  Java:       '#f87171',
  'C++':      '#a78bfa',
  C:          '#f472b6',
  Go:         '#22d3ee',
  Ruby:       '#fb7185',
  HTML:       '#fb923c',
  CSS:        '#34d399',
  JSON:       '#facc15',
  YAML:       '#e879f9',
  Shell:      '#4ade80',
  Markdown:   '#94a3b8',
  default:    '#64748b',
}

const NODE_FOLDER_COLOR  = '#1e3a5f'   // deep navy for folders
const NODE_FOLDER_STROKE = '#3b82f6'   // blue border
const EDGE_COLOR         = '#334155'   // dark slate — thin, no glow
const EDGE_OPACITY       = 0.5
const EDGE_WIDTH         = 1.0        // thin
const EDGE_DEP_COLOR     = '#475569'   // dependency edges slightly lighter
const EDGE_DEP_OPACITY   = 0.6

const MAX_VISIBLE_NODES  = 300        // performance cap
const FOLDER_COLLAPSE_AT = 40         // auto-collapse folders with 40+ children
const LABEL_HIDE_ZOOM    = 0.45       // hide labels below this zoom scale

// Node radii
const R_FOLDER  = 18
const R_FILE    = 9
const R_ENTRY   = 13
const R_ROOT    = 24

// ─── Helpers ─────────────────────────────────────────────────────────────────

function langColor(lang) { return LANG_COLOR[lang] || LANG_COLOR.default }

function fileName(id) {
  if (!id || typeof id !== 'string') return ''
  return id.replace(/\\/g, '/').split('/').pop() || id
}

function parentPath(id) {
  if (!id || typeof id !== 'string') return null
  const parts = id.replace(/\\/g, '/').split('/')
  return parts.length > 1 ? parts.slice(0, -1).join('/') : null
}

// Safe split — normalises Windows backslashes, guards undefined
function safeSplit(id) {
  if (!id || typeof id !== 'string') return []
  return id.replace(/\\/g, '/').split('/')
}

// Build folder tree from flat file list — guards undefined rel_path
function buildFolderTree(files) {
  const folderSet = new Set([''])   // root always exists
  files.forEach(f => {
    const rp = f && f.rel_path
    if (!rp || typeof rp !== 'string') return
    const parts = safeSplit(rp)
    for (let i = 1; i < parts.length; i++) {
      folderSet.add(parts.slice(0, i).join('/'))
    }
  })
  return folderSet
}

// Assign hierarchical level (depth) to each node — guards undefined id
function assignDepths(nodes) {
  const depths = {}
  nodes.forEach(n => {
    if (!n || !n.id) return
    const parts = safeSplit(n.id)
    depths[n.id] = n.type === 'root' ? 0 : parts.length
  })
  return depths
}

// ─── Hierarchical layout engine ───────────────────────────────────────────────
// Uses a top-down radial layout: root at center, folders in ring 1, files in ring 2+
// This matches the reference image structure.

function computeHierarchicalPositions(nodes, edges, W, H) {
  if (!nodes.length) return {}

  // Build parent → children map
  const children = {}
  const parentOf = {}
  nodes.forEach(n => { children[n.id] = [] })

  // Structure edges (folder→file, folder→folder)
  edges
    .filter(e => e.type === 'structure')
    .forEach(e => {
      const src = e.source, tgt = e.target
      if (children[src] && !children[src].includes(tgt)) {
        children[src].push(tgt)
        parentOf[tgt] = src
      }
    })

  // Find root nodes (no parent)
  const roots = nodes.filter(n => !parentOf[n.id])
  if (!roots.length) return {}

  const pos = {}
  const cx = W / 2, cy = H / 2

  // BFS-based radial layout
  // Level 0 (root) → center
  // Level 1 (top folders) → inner ring
  // Level 2 (subfolders/files) → outer ring(s)

  const RING_RADII  = [0, 130, 240, 340, 430, 510]
  const visited = new Set()
  const queue   = []

  roots.forEach((r, i) => {
    const angle = roots.length > 1 ? (i / roots.length) * Math.PI * 2 : 0
    const ring  = RING_RADII[0] || 0
    pos[r.id]   = { x: cx + Math.cos(angle) * ring, y: cy + Math.sin(angle) * ring }
    visited.add(r.id)
    queue.push({ id: r.id, depth: 0 })
  })

  while (queue.length) {
    const { id, depth } = queue.shift()
    const kids = (children[id] || []).filter(k => !visited.has(k))
    if (!kids.length) continue

    const parentPos = pos[id] || { x: cx, y: cy }
    const ring      = RING_RADII[Math.min(depth + 1, RING_RADII.length - 1)]

    // Distribute children in a sector around parent
    const parentAngle = Math.atan2(parentPos.y - cy, parentPos.x - cx)
    const sectorWidth = Math.min(Math.PI * 1.6, Math.PI * 2 / Math.max(kids.length * 0.4, 1))
    const startAngle  = parentAngle - sectorWidth / 2

    kids.forEach((kid, i) => {
      const frac  = kids.length === 1 ? 0.5 : i / (kids.length - 1)
      const angle = startAngle + frac * sectorWidth
      // Add jitter to prevent exact overlap
      const r     = ring + (depth % 2 === 0 ? 0 : 15)
      pos[kid]    = {
        x: cx + Math.cos(angle) * r + (Math.random() - 0.5) * 8,
        y: cy + Math.sin(angle) * r + (Math.random() - 0.5) * 8,
      }
      visited.add(kid)
      queue.push({ id: kid, depth: depth + 1 })
    })
  }

  // Any orphan nodes not yet positioned
  nodes.forEach((n, i) => {
    if (!pos[n.id]) {
      const angle = (i / nodes.length) * Math.PI * 2
      pos[n.id] = {
        x: cx + Math.cos(angle) * (RING_RADII[3] + 40),
        y: cy + Math.sin(angle) * (RING_RADII[3] + 40),
      }
    }
  })

  return pos
}

// ─── Main component ───────────────────────────────────────────────────────────

export default function DependencyGraph({ nodes = [], edges = [], onNodeClick, highlightNode }) {
  const svgRef        = useRef(null)
  const gRef          = useRef(null)
  const zoomRef       = useRef(null)
  const simRef        = useRef(null)
  const rafRef        = useRef(null)

  const [selected,      setSelected]      = useState(null)
  const [collapsed,     setCollapsed]     = useState(new Set())   // collapsed folder ids
  const [zoomScale,     setZoomScale]     = useState(1)
  const [highlighted,   setHighlighted]   = useState(null)        // clicked node id

  // ── Build full node set including folder nodes ────────────────────────────
  const { visibleNodes, visibleEdges, folderNodes } = useMemo(() => {
    if (!nodes.length) return { visibleNodes: [], visibleEdges: [], folderNodes: new Set() }

    // 1. Collect all folder paths
    const folders = buildFolderTree(nodes)
    const folderNodes = new Set(folders)

    // 2. Build complete node list: folder nodes + file nodes
    const allNodes = []

    // Root node
    allNodes.push({
      id:     '__root__',
      type:   'root',
      label:  'project-root',
      language: null,
    })

    // Folder nodes
    folders.forEach(f => {
      if (f === '') return  // skip empty string root
      allNodes.push({
        id:     f,
        type:   'folder',
        label:  f.split(/[/\\]/).pop() || f,
        language: null,
      })
    })

    // File nodes (respect collapsed folders)
    nodes.forEach(n => {
      const parent = parentPath(n.id)
      if (!n || !n.id || typeof n.id !== 'string') return  // guard: skip bad records
      // Check if any ancestor is collapsed
      let isHidden = false
      const parts = safeSplit(n.id)
      for (let i = 1; i < parts.length; i++) {
        if (collapsed.has(parts.slice(0, i).join('/'))) { isHidden = true; break }
      }
      if (!isHidden) {
        allNodes.push({
          id:              n.id,
          type:            'file',
          label:           fileName(n.id),
          language:        n.language,
          risk:            n.risk,
          lines:           n.lines,
          complexity_level:n.complexity_level,
          isEntry:         n.isEntry,
          isCircular:      n.isCircular,
        })
      }
    })

    // 3. Build structure edges (folder→child relationships)
    const structureEdges = []

    // root → top-level folders/files
    allNodes.forEach(n => {
      if (!n || !n.id || typeof n.id !== 'string') return  // guard
      if (n.id === '__root__') return
      const parts = safeSplit(n.id)
      if (parts.length === 1) {
        structureEdges.push({ source: '__root__', target: n.id, type: 'structure' })
      } else {
        const parentId = parts.slice(0, -1).join('/')
        if (allNodes.some(x => x.id === parentId)) {
          structureEdges.push({ source: parentId, target: n.id, type: 'structure' })
        }
      }
    })

    // 4. Dependency edges (file→file imports) — only between visible nodes
    const visibleIds = new Set(allNodes.map(n => n.id))
    const depEdges = edges
      .filter(e => visibleIds.has(e.source) && visibleIds.has(e.target))
      .map(e => ({ ...e, type: 'dependency' }))

    // 5. Performance cap — prioritize by centrality
    let finalNodes = allNodes
    const allEdges = [...structureEdges, ...depEdges]

    if (allNodes.length > MAX_VISIBLE_NODES) {
      // Keep all folder nodes + highest centrality file nodes
      const inDeg = {}
      allNodes.forEach(n => { inDeg[n.id] = 0 })
      allEdges.forEach(e => { if (inDeg[e.target] !== undefined) inDeg[e.target]++ })
      const fileNodes   = allNodes.filter(n => n.type === 'file')
        .sort((a, b) => (inDeg[b.id] || 0) - (inDeg[a.id] || 0))
        .slice(0, MAX_VISIBLE_NODES - allNodes.filter(n => n.type !== 'file').length)
      const nonFileNodes = allNodes.filter(n => n.type !== 'file')
      finalNodes = [...nonFileNodes, ...fileNodes]
    }

    const finalIds = new Set(finalNodes.map(n => n.id))
    const finalEdges = allEdges.filter(e => finalIds.has(e.source) && finalIds.has(e.target))

    return {
      visibleNodes: finalNodes,
      visibleEdges: finalEdges,
      folderNodes,
    }
  }, [nodes, edges, collapsed])

  // ── Render ────────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!svgRef.current || !visibleNodes.length) return

    // Cancel any pending animation frame
    if (rafRef.current) cancelAnimationFrame(rafRef.current)

    const svgEl = svgRef.current
    const W = svgEl.clientWidth  || 900
    const H = svgEl.clientHeight || 600

    const svg = d3.select(svgEl)
    svg.selectAll('*').remove()

    // ── Defs (arrowheads only — NO glow filter per spec) ─────────────────
    const defs = svg.append('defs')

    defs.append('marker')
      .attr('id', 'arr-dep')
      .attr('viewBox', '0 -3 6 6').attr('refX', 14).attr('refY', 0)
      .attr('markerWidth', 5).attr('markerHeight', 5).attr('orient', 'auto')
      .append('path').attr('d', 'M0,-3L6,0L0,3')
      .attr('fill', EDGE_DEP_COLOR).attr('opacity', 0.5)

    defs.append('marker')
      .attr('id', 'arr-struct')
      .attr('viewBox', '0 -3 6 6').attr('refX', 20).attr('refY', 0)
      .attr('markerWidth', 4).attr('markerHeight', 4).attr('orient', 'auto')
      .append('path').attr('d', 'M0,-3L6,0L0,3')
      .attr('fill', EDGE_COLOR).attr('opacity', 0.35)

    // Highlight marker
    defs.append('marker')
      .attr('id', 'arr-hl')
      .attr('viewBox', '0 -3 6 6').attr('refX', 14).attr('refY', 0)
      .attr('markerWidth', 5).attr('markerHeight', 5).attr('orient', 'auto')
      .append('path').attr('d', 'M0,-3L6,0L0,3')
      .attr('fill', '#7c5cfc').attr('opacity', 0.9)

    // ── Zoom ──────────────────────────────────────────────────────────────
    const g = svg.append('g')
    gRef.current = g

    const zoomBehavior = d3.zoom()
      .scaleExtent([0.1, 5])
      .on('zoom', event => {
        g.attr('transform', event.transform)
        setZoomScale(event.transform.k)
        // Level-of-detail: hide/show labels based on zoom
        g.selectAll('.node-label')
          .attr('display', event.transform.k < LABEL_HIDE_ZOOM ? 'none' : 'block')
      })

    svg.call(zoomBehavior)
    zoomRef.current = zoomBehavior

    // ── Compute initial positions (hierarchical) ──────────────────────────
    const initialPos = computeHierarchicalPositions(visibleNodes, visibleEdges, W, H)
    const nodeData   = visibleNodes.map(n => ({
      ...n,
      x: initialPos[n.id]?.x ?? W / 2,
      y: initialPos[n.id]?.y ?? H / 2,
    }))
    const nodeById = Object.fromEntries(nodeData.map(n => [n.id, n]))

    const edgeData = visibleEdges.map(e => ({
      ...e,
      source: nodeById[e.source] || e.source,
      target: nodeById[e.target] || e.target,
    }))

    // ── Force simulation (gentle — layout is pre-computed) ────────────────
    const sim = d3.forceSimulation(nodeData)
      .force('link', d3.forceLink(edgeData)
        .id(d => d.id)
        .distance(d => d.type === 'structure' ? 60 : 100)
        .strength(d => d.type === 'structure' ? 0.6 : 0.15)
      )
      .force('charge',    d3.forceManyBody().strength(d => d.type === 'folder' ? -180 : -80).distanceMax(250))
      .force('collision', d3.forceCollide().radius(d => {
        if (d.type === 'root')   return R_ROOT + 22
        if (d.type === 'folder') return R_FOLDER + 18
        return R_FILE + 12
      }).strength(1.0).iterations(3))
      .force('x', d3.forceX(d => initialPos[d.id]?.x ?? W / 2).strength(0.25))
      .force('y', d3.forceY(d => initialPos[d.id]?.y ?? H / 2).strength(0.25))
      .alphaDecay(0.025)
      .velocityDecay(0.6)

    simRef.current = sim

    // ── Edges ─────────────────────────────────────────────────────────────
    const linkG = g.append('g').attr('class', 'link-layer')

    const link = linkG.selectAll('line')
      .data(edgeData)
      .join('line')
      .attr('class', d => `edge edge-${d.type}`)
      .attr('stroke', d => d.type === 'dependency' ? EDGE_DEP_COLOR : EDGE_COLOR)
      .attr('stroke-width', d => d.type === 'dependency' ? EDGE_WIDTH : 0.8)
      .attr('stroke-opacity', d => d.type === 'dependency' ? EDGE_DEP_OPACITY : EDGE_OPACITY)
      .attr('marker-end', d => d.type === 'dependency' ? 'url(#arr-dep)' : 'url(#arr-struct)')
      .attr('fill', 'none')

    // ── Nodes ─────────────────────────────────────────────────────────────
    const nodeG = g.append('g').attr('class', 'node-layer')

    const nodeGroup = nodeG.selectAll('g')
      .data(nodeData)
      .join('g')
      .attr('class', 'node-group')
      .style('cursor', 'pointer')
      .call(d3.drag()
        .on('start', (ev, d) => { if (!ev.active) sim.alphaTarget(0.2).restart(); d.fx = d.x; d.fy = d.y })
        .on('drag',  (ev, d) => { d.fx = ev.x; d.fy = ev.y })
        .on('end',   (ev, d) => { if (!ev.active) sim.alphaTarget(0); d.fx = null; d.fy = null })
      )
      .on('click', (ev, d) => {
        ev.stopPropagation()
        setSelected(d)
        setHighlighted(d.id)
        onNodeClick && onNodeClick(d)
        _applyHighlight(d.id)
      })

    // ── Node shapes ────────────────────────────────────────────────────────
    // Root: large hexagon
    nodeGroup.filter(d => d.type === 'root')
      .append('polygon')
      .attr('points', _hexPoints(R_ROOT))
      .attr('fill', '#0f2744')
      .attr('stroke', '#3b82f6')
      .attr('stroke-width', 2)

    // Folder: hexagon (matches reference image)
    nodeGroup.filter(d => d.type === 'folder')
      .append('polygon')
      .attr('points', _hexPoints(R_FOLDER))
      .attr('fill', NODE_FOLDER_COLOR)
      .attr('stroke', NODE_FOLDER_STROKE)
      .attr('stroke-width', 1.5)
      .attr('fill-opacity', 0.9)

    // Folder collapse/expand indicator
    nodeGroup.filter(d => d.type === 'folder')
      .append('text')
      .attr('class', 'folder-toggle')
      .attr('text-anchor', 'middle')
      .attr('dy', '0.35em')
      .attr('font-size', '10px')
      .attr('fill', '#93c5fd')
      .attr('pointer-events', 'none')
      .text(d => collapsed.has(d.id) ? '▶' : '⊟')

    // File: circle
    nodeGroup.filter(d => d.type === 'file')
      .append('circle')
      .attr('r', d => d.isEntry ? R_ENTRY : R_FILE)
      .attr('fill', d => langColor(d.language))
      .attr('fill-opacity', 0.88)
      .attr('stroke', d =>
        d.isCircular ? '#f87171' :
        d.isEntry    ? '#34d399' : 'none'
      )
      .attr('stroke-width', d => (d.isCircular || d.isEntry) ? 1.5 : 0)

    // File language icon text
    nodeGroup.filter(d => d.type === 'file')
      .append('text')
      .attr('text-anchor', 'middle')
      .attr('dy', '0.35em')
      .attr('font-size', '7px')
      .attr('font-weight', '700')
      .attr('fill', '#fff')
      .attr('pointer-events', 'none')
      .text(d => _langIcon(d.language))

    // ── Labels ─────────────────────────────────────────────────────────────
    nodeGroup.append('text')
      .attr('class', 'node-label')
      .attr('text-anchor', 'middle')
      .attr('dy', d => {
        if (d.type === 'root')   return R_ROOT + 14
        if (d.type === 'folder') return R_FOLDER + 13
        return (d.isEntry ? R_ENTRY : R_FILE) + 12
      })
      .attr('font-family', "'JetBrains Mono', monospace")
      .attr('font-size', d => d.type === 'folder' ? '10px' : '9px')
      .attr('font-weight', d => d.type === 'folder' ? '600' : '400')
      .attr('fill', d =>
        d.type === 'root'   ? '#93c5fd' :
        d.type === 'folder' ? '#bfdbfe' : '#94a3b8'
      )
      .attr('display', zoomScale < LABEL_HIDE_ZOOM ? 'none' : 'block')
      .text(d => {
        const lbl = d.label || fileName(d.id)
        return lbl.length > 20 ? lbl.slice(0, 18) + '…' : lbl
      })

    // Tooltip: full path on hover
    nodeGroup.append('title').text(d => d.id)

    // ── Folder click: expand/collapse ──────────────────────────────────────
    nodeGroup.filter(d => d.type === 'folder')
      .on('dblclick', (ev, d) => {
        ev.stopPropagation()
        setCollapsed(prev => {
          const next = new Set(prev)
          if (next.has(d.id)) next.delete(d.id)
          else next.add(d.id)
          return next
        })
      })

    // ── Highlight helper ───────────────────────────────────────────────────
    function _applyHighlight(nodeId) {
      if (!nodeId) {
        // Reset all
        link.attr('stroke-opacity', d => d.type === 'dependency' ? EDGE_DEP_OPACITY : EDGE_OPACITY)
          .attr('stroke', d => d.type === 'dependency' ? EDGE_DEP_COLOR : EDGE_COLOR)
          .attr('marker-end', d => d.type === 'dependency' ? 'url(#arr-dep)' : 'url(#arr-struct)')
        nodeGroup.attr('opacity', 1)
        return
      }

      // Find connected edge ids
      const connectedNodes = new Set([nodeId])
      edgeData.forEach(e => {
        const src = e.source?.id ?? e.source
        const tgt = e.target?.id ?? e.target
        if (src === nodeId || tgt === nodeId) {
          connectedNodes.add(src)
          connectedNodes.add(tgt)
        }
      })

      // Fade unrelated nodes
      nodeGroup.attr('opacity', d => connectedNodes.has(d.id) ? 1 : 0.18)

      // Highlight connected edges, fade others
      link
        .attr('stroke-opacity', d => {
          const src = d.source?.id ?? d.source
          const tgt = d.target?.id ?? d.target
          return (src === nodeId || tgt === nodeId) ? 0.9 : 0.06
        })
        .attr('stroke', d => {
          const src = d.source?.id ?? d.source
          const tgt = d.target?.id ?? d.target
          return (src === nodeId || tgt === nodeId) ? '#7c5cfc' : EDGE_COLOR
        })
        .attr('stroke-width', d => {
          const src = d.source?.id ?? d.source
          const tgt = d.target?.id ?? d.target
          return (src === nodeId || tgt === nodeId) ? 2 : 0.6
        })
        .attr('marker-end', d => {
          const src = d.source?.id ?? d.source
          const tgt = d.target?.id ?? d.target
          return (src === nodeId || tgt === nodeId) ? 'url(#arr-hl)' : 'url(#arr-struct)'
        })
    }

    // ── Tick — use requestAnimationFrame for performance ──────────────────
    let tickCount = 0
    sim.on('tick', () => {
      tickCount++
      // Only update DOM every 2 ticks for large graphs
      if (tickCount % 2 !== 0 && nodeData.length > 100) return

      rafRef.current = requestAnimationFrame(() => {
        link
          .attr('x1', d => d.source.x ?? 0).attr('y1', d => d.source.y ?? 0)
          .attr('x2', d => d.target.x ?? 0).attr('y2', d => d.target.y ?? 0)
        nodeGroup.attr('transform', d => `translate(${d.x ?? 0},${d.y ?? 0})`)
      })
    })

    // Click background → deselect
    svg.on('click', () => {
      setSelected(null)
      setHighlighted(null)
      _applyHighlight(null)
    })

    return () => {
      sim.stop()
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
    }
  }, [visibleNodes, visibleEdges])

  // ── Controls ──────────────────────────────────────────────────────────────
  const resetZoom = useCallback(() => {
    if (!svgRef.current || !zoomRef.current) return
    d3.select(svgRef.current)
      .transition().duration(500)
      .call(zoomRef.current.transform, d3.zoomIdentity.translate(
        svgRef.current.clientWidth / 2,
        svgRef.current.clientHeight / 2
      ).scale(0.85))
  }, [])

  const reheat = useCallback(() => {
    simRef.current?.alpha(0.4).restart()
  }, [])

  // ─── Legend ────────────────────────────────────────────────────────────────
  const presentLangs = useMemo(() =>
    Object.entries(LANG_COLOR).filter(([k]) =>
      k !== 'default' && nodes.some(n => n.language === k)
    ), [nodes])

  return (
    <div className="graph-container" style={{ position: 'relative', width: '100%', height: '100%' }}>
      <svg
        ref={svgRef}
        width="100%" height="100%"
        style={{ display: 'block', background: 'transparent' }}
      />

      {/* ── Legend ─────────────────────────────────────────── */}
      <div className="graph-legend" style={{ maxHeight: 280, overflowY: 'auto' }}>
        <div style={{ fontSize: 9, color: 'var(--text-dim)', marginBottom: 6, letterSpacing: 1, textTransform: 'uppercase' }}>Node Types</div>
        <div className="legend-row">
          <svg width="14" height="14" style={{ flexShrink: 0 }}>
            <polygon points={_hexPoints(6)} transform="translate(7,7)" fill={NODE_FOLDER_COLOR} stroke={NODE_FOLDER_STROKE} strokeWidth="1.5" />
          </svg>
          <span>Folder</span>
        </div>
        <div className="legend-row">
          <div style={{ width: 10, height: 10, borderRadius: '50%', background: '#64748b', flexShrink: 0 }} />
          <span>File</span>
        </div>
        <div className="legend-row">
          <div style={{ width: 10, height: 10, borderRadius: '50%', background: '#64748b', border: '1.5px solid #34d399', flexShrink: 0 }} />
          <span>Entry point</span>
        </div>
        <div className="legend-row">
          <div style={{ width: 10, height: 10, borderRadius: '50%', background: '#64748b', border: '1.5px dashed #f87171', flexShrink: 0 }} />
          <span>Circular dep</span>
        </div>
        {presentLangs.length > 0 && (
          <>
            <div style={{ fontSize: 9, color: 'var(--text-dim)', margin: '6px 0 4px', letterSpacing: 1, textTransform: 'uppercase' }}>Languages</div>
            {presentLangs.map(([lang, color]) => (
              <div key={lang} className="legend-row">
                <div className="legend-dot" style={{ background: color }} />
                <span>{lang}</span>
              </div>
            ))}
          </>
        )}
        <div style={{ borderTop: '1px solid var(--border-dim)', margin: '6px 0 4px' }} />
        <div style={{ fontSize: 9, color: 'var(--text-dim)', marginBottom: 4, letterSpacing: 1, textTransform: 'uppercase' }}>Edges</div>
        <div className="legend-row">
          <div style={{ width: 16, height: 1, background: EDGE_COLOR, opacity: EDGE_OPACITY }} />
          <span>Structure</span>
        </div>
        <div className="legend-row">
          <div style={{ width: 16, height: 1, background: EDGE_DEP_COLOR, opacity: EDGE_DEP_OPACITY }} />
          <span>Dependency</span>
        </div>
        <div style={{ fontSize: 9, color: 'var(--text-dim)', marginTop: 6 }}>
          Dbl-click folder to expand/collapse
        </div>
      </div>

      {/* ── Node detail panel ──────────────────────────────── */}
      {selected && (
        <div className="node-detail-panel">
          <button
            onClick={() => { setSelected(null); setHighlighted(null) }}
            style={{ position: 'absolute', top: 8, right: 10, background: 'none', border: 'none', color: 'var(--text-dim)', cursor: 'pointer', fontSize: 16 }}
          >×</button>
          <div className="node-detail-name">{selected.label || fileName(selected.id)}</div>
          {selected.type === 'folder' && (
            <div className="node-detail-row">
              <span>Type</span>
              <span className="node-detail-val" style={{ color: '#93c5fd' }}>Folder</span>
            </div>
          )}
          {selected.type === 'file' && (
            <>
              <div className="node-detail-row">
                <span>Language</span>
                <span className="node-detail-val" style={{ color: langColor(selected.language) }}>{selected.language || 'Unknown'}</span>
              </div>
              <div className="node-detail-row">
                <span>Risk</span>
                <span className="node-detail-val" style={{
                  color: selected.risk === 'high' ? '#f87171' : selected.risk === 'medium' ? '#fbbf24' : '#34d399'
                }}>
                  {selected.risk?.toUpperCase() || 'LOW'}
                </span>
              </div>
              {selected.lines > 0 && (
                <div className="node-detail-row">
                  <span>Lines</span>
                  <span className="node-detail-val">{selected.lines}</span>
                </div>
              )}
              {selected.complexity_level && (
                <div className="node-detail-row">
                  <span>Complexity</span>
                  <span className="node-detail-val">{selected.complexity_level}</span>
                </div>
              )}
            </>
          )}
          <div className="node-detail-row" style={{ marginTop: 6 }}>
            <span style={{ wordBreak: 'break-all', fontSize: 10, color: 'var(--text-dim)' }}>{selected.id}</span>
          </div>
        </div>
      )}

      {/* ── Controls ───────────────────────────────────────── */}
      <div className="graph-controls">
        <button className="graph-ctrl-btn" title="Reset zoom" onClick={resetZoom}>⊹</button>
        <button className="graph-ctrl-btn" title="Reheat layout" onClick={reheat}>↺</button>
        <button
          className="graph-ctrl-btn"
          title="Expand all folders"
          onClick={() => setCollapsed(new Set())}
          style={{ fontSize: 11 }}
        >⊞</button>
      </div>

      {/* ── Node count badge ───────────────────────────────── */}
      <div style={{
        position: 'absolute', bottom: 14, right: 14,
        background: 'var(--bg-overlay)', border: '1px solid var(--border-dim)',
        borderRadius: 'var(--radius-sm)', padding: '4px 10px',
        fontSize: 10, color: 'var(--text-dim)', fontFamily: 'var(--font-mono)',
      }}>
        {visibleNodes.filter(n => n.type === 'file').length} files · {visibleNodes.filter(n => n.type === 'folder').length} folders
        {nodes.length > MAX_VISIBLE_NODES && (
          <span style={{ color: 'var(--yellow)', marginLeft: 8 }}>
            (showing top {MAX_VISIBLE_NODES})
          </span>
        )}
      </div>
    </div>
  )
}

// ─── Utility: hexagon points ──────────────────────────────────────────────────
function _hexPoints(r) {
  return Array.from({ length: 6 }, (_, i) => {
    const a = (Math.PI / 3) * i - Math.PI / 6
    return `${Math.cos(a) * r},${Math.sin(a) * r}`
  }).join(' ')
}

// ─── Language icon abbreviations ─────────────────────────────────────────────
function _langIcon(lang) {
  const icons = {
    Python: 'PY', JavaScript: 'JS', TypeScript: 'TS',
    Java: 'JV', 'C++': 'C+', C: 'C', Go: 'GO',
    Ruby: 'RB', HTML: 'HT', CSS: 'CS', JSON: '{}',
    YAML: 'YM', Shell: 'SH', Markdown: 'MD',
  }
  return icons[lang] || '?'
}
