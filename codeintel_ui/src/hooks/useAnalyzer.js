// src/hooks/useAnalyzer.js
import { useState, useCallback } from 'react'

const API = 'http://localhost:8000'

const STAGES = [
  { id: 'scan',   label: 'Scanning files' },
  { id: 'deps',   label: 'Building dependency graph' },
  { id: 'errors', label: 'Detecting issues' },
  { id: 'trace',  label: 'Tracing execution' },
  { id: 'risk',   label: 'Scoring risk' },
  { id: 'graph',  label: 'Rendering graph' },
]

export function useAnalyzer() {
  const [data,     setData]     = useState(null)
  const [loading,  setLoading]  = useState(false)
  const [error,    setError]    = useState(null)
  const [stage,    setStage]    = useState(0)   // 0-5 stage index
  const [status,   setStatus]   = useState('idle')

  const analyze = useCallback(async (path, opts = {}) => {
    if (!path.trim()) { setError('Enter a project path'); return }
    setLoading(true)
    setError(null)
    setData(null)
    setStage(0)
    setStatus('running')

    // Simulate stage progression while waiting for response
    let stageIdx = 0
    const stageInterval = setInterval(() => {
      stageIdx = Math.min(stageIdx + 1, STAGES.length - 1)
      setStage(stageIdx)
    }, 1200)

    try {
      const res = await fetch(`${API}/analyze`, {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({
          path:             path.trim(),
          safe_exec:        opts.safeExec        ?? false,
          detect_duplicates:opts.detectDuplicates ?? false,
        }),
      })

      clearInterval(stageInterval)

      if (!res.ok) {
        const err = await res.json().catch(() => ({ detail: 'Server error' }))
        throw new Error(err.detail || `HTTP ${res.status}`)
      }

      const json = await res.json()
      setData(json)
      setStage(STAGES.length)
      setStatus('done')
    } catch (e) {
      clearInterval(stageInterval)
      setError(e.message.includes('fetch') || e.message.includes('Failed')
        ? 'Cannot reach backend. Run: uvicorn server:app --reload --port 8000'
        : e.message
      )
      setStatus('error')
    } finally {
      setLoading(false)
    }
  }, [])

  const reset = useCallback(() => {
    setData(null); setError(null); setStatus('idle'); setStage(0)
  }, [])

  return { data, loading, error, stage, stages: STAGES, status, analyze, reset }
}

// ── AI chat hook ──────────────────────────────────────────────────────────────
export function useChat(projectData) {
  const [messages,  setMessages]  = useState([])
  const [thinking,  setThinking]  = useState(false)

  const sendMessage = useCallback(async (text, contextFile = null) => {
    if (!text.trim() || thinking) return
    const userMsg = { role: 'user', content: text, ts: new Date().toLocaleTimeString() }
    setMessages(prev => [...prev, userMsg])
    setThinking(true)

    try {
      const res = await fetch(`${API}/chat`, {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({
          question:     text,
          context_file: contextFile,
          // API key intentionally omitted — server reads from NVIDIA_API_KEY env var
          history:      messages.slice(-6).map(m => ({ role: m.role, content: m.content })),
        }),
      })

      if (!res.ok) {
        const err = await res.json().catch(() => ({ detail: 'AI error' }))
        throw new Error(err.detail || 'AI request failed')
      }

      // Streaming response
      const reader = res.body.getReader()
      const decoder = new TextDecoder()
      let aiText = ''
      const aiMsg = { role: 'ai', content: '', ts: new Date().toLocaleTimeString(), streaming: true }
      setMessages(prev => [...prev, aiMsg])

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        const chunk = decoder.decode(value, { stream: true })
        const lines = chunk.split('\n').filter(l => l.startsWith('data: '))
        for (const line of lines) {
          const raw = line.slice(6)
          if (raw === '[DONE]') continue
          try {
            const parsed = JSON.parse(raw)
            if (parsed.content) {
              aiText += parsed.content
              setMessages(prev => {
                const updated = [...prev]
                updated[updated.length - 1] = { ...aiMsg, content: aiText, streaming: true }
                return updated
              })
            }
            if (parsed.confidence !== undefined) {
              setMessages(prev => {
                const updated = [...prev]
                updated[updated.length - 1] = {
                  ...aiMsg, content: aiText, streaming: false,
                  confidence: parsed.confidence
                }
                return updated
              })
            }
          } catch { /* ignore parse errors */ }
        }
      }

      setMessages(prev => {
        const updated = [...prev]
        if (updated.length > 0) {
          updated[updated.length - 1] = { ...updated[updated.length - 1], streaming: false }
        }
        return updated
      })

    } catch (e) {
      setMessages(prev => [...prev, {
        role: 'error', content: e.message,
        ts: new Date().toLocaleTimeString()
      }])
    } finally {
      setThinking(false)
    }
  }, [messages, thinking])

  const clearChat = () => setMessages([])

  return { messages, thinking, sendMessage, clearChat }
}
