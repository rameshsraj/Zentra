import { useState, useRef, useEffect, useCallback } from 'react'
import { mcpChat, getMcpTools, type McpChatResponse, type McpToolDef } from '@/lib/api'
import {
  MessageSquare, Send, Bot, User, Loader2, Trash2, X, Minimize2,
  Maximize2, Settings2, Wrench, Zap, Globe, Database,
  Server, Network, Shield, HardDrive, BarChart3, Terminal,
} from 'lucide-react'
import { cn } from '@/lib/utils'

interface ChatMsg {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
  mode_used?: string
  tool_calls?: McpChatResponse['tool_calls']
  suggestions?: string[]
}

type ChatMode = 'mcp' | 'llm' | 'api'

const modeConfig: Record<ChatMode, { label: string; icon: typeof Zap; color: string; desc: string }> = {
  mcp:  { label: 'MCP Server', icon: Zap,    color: 'text-brand-400',   desc: 'Real-time infrastructure data via MCP tools' },
  llm:  { label: 'LLM',        icon: Bot,    color: 'text-purple-400',  desc: 'AI language model fallback' },
  api:  { label: 'Direct API', icon: Globe,  color: 'text-amber-400',   desc: 'Raw API endpoint guidance' },
}

const categoryIcons: Record<string, typeof Server> = {
  'Server': Server,
  'Oracle Database': Database,
  'Network': Network,
  'Storage': HardDrive,
  'Alerts': Shield,
  'Logs': Terminal,
  'Metrics': BarChart3,
  'Predictions': BarChart3,
  'System': Settings2,
}

export default function FloatingChatbot() {
  const [open, setOpen] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [input, setInput] = useState('')
  const [isTyping, setIsTyping] = useState(false)
  const [mode, setMode] = useState<ChatMode>('mcp')
  const [showModeMenu, setShowModeMenu] = useState(false)
  const [showTools, setShowTools] = useState(false)
  const [tools, setTools] = useState<McpToolDef[]>([])
  const [sessionId] = useState(() => `session-${Date.now()}`)
  const endRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const [messages, setMessages] = useState<ChatMsg[]>([{
    id: 'welcome',
    role: 'assistant',
    content: '**Zentra MCP Assistant**\n\nI have access to 35+ infrastructure tools via MCP. Ask me about:\n\n- **Oracle DB** — tablespaces, RAC, Data Guard, RMAN, wait events, long queries\n- **Servers** — CPU, memory, disk, processes, health\n- **Network** — traffic, bandwidth, protocols, threats\n- **Storage** — arrays, capacity forecasts, IOPS\n- **Alerts** — active alerts, silences, history\n- **Logs** — search, error analysis\n- **Predictions** — anomalies, forecasts\n\nWhat would you like to investigate?',
    timestamp: new Date(),
    mode_used: 'mcp',
  }])

  // Load MCP tools on first open
  useEffect(() => {
    if (open && tools.length === 0) {
      getMcpTools().then(setTools).catch(() => {})
    }
  }, [open, tools.length])

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  useEffect(() => {
    if (open) setTimeout(() => inputRef.current?.focus(), 100)
  }, [open])

  const sendMessage = useCallback(async (text: string) => {
    if (!text.trim() || isTyping) return

    const userMsg: ChatMsg = { id: `u-${Date.now()}`, role: 'user', content: text, timestamp: new Date() }
    setMessages(prev => [...prev, userMsg])
    setInput('')
    setIsTyping(true)

    try {
      const response = await mcpChat(text, sessionId, mode)
      const assistantMsg: ChatMsg = {
        id: `a-${Date.now()}`,
        role: 'assistant',
        content: response.text,
        timestamp: new Date(),
        mode_used: response.mode_used,
        tool_calls: response.tool_calls,
        suggestions: response.suggestions,
      }
      setMessages(prev => [...prev, assistantMsg])
    } catch {
      setMessages(prev => [...prev, {
        id: `e-${Date.now()}`,
        role: 'assistant',
        content: 'Sorry, I encountered an error connecting to the MCP server. Please check if the backend is running.',
        timestamp: new Date(),
        mode_used: 'error',
      }])
    }
    setIsTyping(false)
  }, [isTyping, sessionId, mode])

  const clearChat = () => setMessages([messages[0]])

  const renderMarkdown = (text: string) => {
    // Sanitize HTML entities first to prevent XSS injection
    const escaped = text
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
    return escaped
      .replace(/\*\*(.*?)\*\*/g, '<strong class="text-white">$1</strong>')
      .replace(/`([^`]+)`/g, '<code class="bg-white/10 px-1 py-0.5 rounded text-brand-400 text-xs font-mono">$1</code>')
      .replace(/```(\w+)?\n([\s\S]*?)```/g, '<pre class="bg-black/30 rounded-lg p-3 mt-2 mb-2 overflow-x-auto text-xs font-mono text-slate-300"><code>$2</code></pre>')
      .replace(/^- (.*)/gm, '<span class="flex items-start gap-1.5"><span class="text-brand-400 mt-0.5">•</span><span>$1</span></span>')
  }

  // Grouped tools by category
  const toolsByCategory = tools.reduce<Record<string, McpToolDef[]>>((acc, t) => {
    (acc[t.category] = acc[t.category] || []).push(t)
    return acc
  }, {})

  if (!open) {
    return (
      <button
        onClick={() => setOpen(true)}
        className="fixed bottom-6 right-6 z-50 w-14 h-14 bg-brand-600 hover:bg-brand-500 rounded-full shadow-2xl shadow-brand-600/30 flex items-center justify-center transition-all hover:scale-110 group"
        title="Open MCP Assistant (AI Chatbot)"
      >
        <MessageSquare className="w-6 h-6 text-white" />
        <span className="absolute -top-1 -right-1 w-4 h-4 bg-emerald-500 rounded-full border-2 border-slate-900 animate-pulse" />
      </button>
    )
  }

  return (
    <div className={cn(
      'fixed z-50 flex flex-col bg-slate-900/95 backdrop-blur-xl border border-white/10 shadow-2xl transition-all duration-300',
      expanded
        ? 'inset-4 rounded-2xl'
        : 'bottom-6 right-6 w-[420px] h-[600px] rounded-2xl'
    )}>
      {/* Header */}
      <div className="flex items-center gap-3 px-4 py-3 border-b border-white/10 flex-shrink-0">
        <div className="w-8 h-8 rounded-lg bg-brand-600/20 flex items-center justify-center">
          <Bot className="w-4 h-4 text-brand-400" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-bold text-white">MCP Assistant</div>
          <div className="flex items-center gap-1.5 text-[10px]">
            <span className={modeConfig[mode].color}>{modeConfig[mode].label}</span>
            <span className="text-slate-600">·</span>
            <span className="text-slate-500">{tools.length} tools</span>
          </div>
        </div>
        <div className="flex items-center gap-1">
          <button onClick={() => setShowTools(!showTools)} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors" title="MCP Tools">
            <Wrench className="w-3.5 h-3.5" />
          </button>
          <div className="relative">
            <button onClick={() => setShowModeMenu(!showModeMenu)} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors" title="Switch mode">
              <Settings2 className="w-3.5 h-3.5" />
            </button>
            {showModeMenu && (
              <div className="absolute right-0 top-full mt-1 w-52 bg-slate-800 border border-white/10 rounded-lg shadow-xl z-10 py-1">
                {(Object.entries(modeConfig) as [ChatMode, typeof modeConfig['mcp']][]).map(([key, cfg]) => (
                  <button
                    key={key}
                    onClick={() => { setMode(key); setShowModeMenu(false) }}
                    className={cn('w-full flex items-center gap-2 px-3 py-2 text-xs transition-colors', mode === key ? 'bg-white/10 text-white' : 'text-slate-400 hover:bg-white/5 hover:text-white')}
                  >
                    <cfg.icon className={cn('w-3.5 h-3.5', cfg.color)} />
                    <div className="text-left">
                      <div className="font-medium">{cfg.label}</div>
                      <div className="text-[10px] text-slate-500">{cfg.desc}</div>
                    </div>
                    {mode === key && <span className="ml-auto w-1.5 h-1.5 rounded-full bg-brand-400" />}
                  </button>
                ))}
              </div>
            )}
          </div>
          <button onClick={clearChat} className="p-1.5 rounded-lg text-slate-400 hover:text-red-400 hover:bg-white/10 transition-colors" title="Clear">
            <Trash2 className="w-3.5 h-3.5" />
          </button>
          <button onClick={() => setExpanded(!expanded)} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors" title={expanded ? 'Minimize' : 'Expand'}>
            {expanded ? <Minimize2 className="w-3.5 h-3.5" /> : <Maximize2 className="w-3.5 h-3.5" />}
          </button>
          <button onClick={() => { setOpen(false); setExpanded(false) }} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors" title="Close">
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Tools Panel */}
      {showTools && (
        <div className="border-b border-white/10 px-4 py-3 max-h-48 overflow-y-auto flex-shrink-0 custom-scrollbar">
          <div className="text-[10px] font-bold text-slate-500 uppercase mb-2">MCP Tool Registry ({tools.length} tools)</div>
          {Object.entries(toolsByCategory).map(([cat, catTools]) => {
            const CatIcon = categoryIcons[cat] || Wrench
            return (
              <div key={cat} className="mb-2">
                <div className="flex items-center gap-1.5 text-[10px] text-slate-400 font-medium mb-1">
                  <CatIcon className="w-3 h-3 text-brand-400" />{cat}
                </div>
                <div className="grid grid-cols-2 gap-1 ml-4">
                  {catTools.map(t => (
                    <div key={t.name} className="bg-white/5 rounded px-2 py-1 text-[9px]">
                      <span className="font-mono text-slate-300">{t.name}</span>
                    </div>
                  ))}
                </div>
              </div>
            )
          })}
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-3 custom-scrollbar">
        {messages.map(msg => (
          <div key={msg.id} className={cn('flex gap-2', msg.role === 'user' && 'justify-end')}>
            {msg.role === 'assistant' && (
              <div className="w-7 h-7 rounded-lg bg-brand-600/20 flex items-center justify-center flex-shrink-0 mt-0.5">
                <Bot className="w-3.5 h-3.5 text-brand-400" />
              </div>
            )}
            <div className={cn(
              'max-w-[85%] rounded-xl p-3 text-[13px] leading-relaxed',
              msg.role === 'user'
                ? 'bg-brand-600/20 text-white border border-brand-500/20'
                : 'bg-white/5 text-slate-200 border border-white/10'
            )}>
              {/* Mode badge */}
              {msg.mode_used && msg.role === 'assistant' && (
                <div className="flex items-center gap-1.5 mb-1.5 text-[9px]">
                  {msg.mode_used === 'mcp' && <Zap className="w-3 h-3 text-brand-400" />}
                  {msg.mode_used === 'llm' && <Bot className="w-3 h-3 text-purple-400" />}
                  {msg.mode_used === 'api' && <Globe className="w-3 h-3 text-amber-400" />}
                  <span className={cn(
                    msg.mode_used === 'mcp' && 'text-brand-400',
                    msg.mode_used === 'llm' && 'text-purple-400',
                    msg.mode_used === 'api' && 'text-amber-400',
                    msg.mode_used === 'error' && 'text-red-400',
                  )}>{msg.mode_used.toUpperCase()}</span>
                </div>
              )}

              {/* Tool calls */}
              {msg.tool_calls && msg.tool_calls.length > 0 && (
                <div className="flex flex-wrap gap-1 mb-2">
                  {msg.tool_calls.map((tc, i) => (
                    <span key={i} className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-emerald-500/10 border border-emerald-500/20 rounded text-[9px] text-emerald-400 font-mono">
                      <Terminal className="w-2.5 h-2.5" />{tc.tool} <span className="text-emerald-600">{tc.execution_ms}ms</span>
                    </span>
                  ))}
                </div>
              )}

              <div className="whitespace-pre-wrap" dangerouslySetInnerHTML={{ __html: renderMarkdown(msg.content) }} />

              {/* Tool call details */}
              {msg.tool_calls && msg.tool_calls.length > 0 && (
                <details className="mt-2">
                  <summary className="text-[9px] text-slate-500 cursor-pointer hover:text-slate-300">View raw MCP data</summary>
                  <pre className="mt-1 bg-black/30 rounded-lg p-2 text-[9px] font-mono text-slate-400 overflow-x-auto max-h-32 overflow-y-auto">
                    {JSON.stringify(msg.tool_calls.map(tc => ({ tool: tc.tool, data: tc.data })), null, 2)}
                  </pre>
                </details>
              )}

              {/* Timestamp */}
              <div className="text-[9px] text-slate-600 mt-1.5">{msg.timestamp.toLocaleTimeString()}</div>
            </div>
            {msg.role === 'user' && (
              <div className="w-7 h-7 rounded-lg bg-slate-700 flex items-center justify-center flex-shrink-0 mt-0.5">
                <User className="w-3.5 h-3.5 text-slate-300" />
              </div>
            )}
          </div>
        ))}

        {/* Suggestions from last message */}
        {!isTyping && messages.length > 0 && messages[messages.length - 1].suggestions && (
          <div className="flex flex-wrap gap-1.5 pl-9">
            {messages[messages.length - 1].suggestions!.map((s, i) => (
              <button key={i} onClick={() => sendMessage(s)} className="px-2.5 py-1 bg-white/5 border border-white/10 rounded-lg text-[10px] text-slate-400 hover:text-white hover:bg-white/10 transition-colors">
                {s}
              </button>
            ))}
          </div>
        )}

        {isTyping && (
          <div className="flex gap-2">
            <div className="w-7 h-7 rounded-lg bg-brand-600/20 flex items-center justify-center flex-shrink-0">
              <Bot className="w-3.5 h-3.5 text-brand-400" />
            </div>
            <div className="bg-white/5 border border-white/10 rounded-xl p-3 flex items-center gap-2">
              <Loader2 className="w-3.5 h-3.5 text-brand-400 animate-spin" />
              <span className="text-[10px] text-slate-500">Querying MCP tools...</span>
            </div>
          </div>
        )}
        <div ref={endRef} />
      </div>

      {/* Quick suggestions on first load */}
      {messages.length <= 1 && (
        <div className="px-4 pb-2 flex flex-wrap gap-1.5 flex-shrink-0">
          {[
            'Show Oracle tablespace usage',
            'Check all server health',
            'Any critical alerts?',
            'Show network traffic',
            'Oracle RAC cluster status',
            'Storage capacity forecast',
          ].map((s, i) => (
            <button key={i} onClick={() => sendMessage(s)} className="px-2.5 py-1 bg-white/5 border border-white/10 rounded-lg text-[10px] text-slate-400 hover:text-white hover:bg-white/10 transition-colors">
              {s}
            </button>
          ))}
        </div>
      )}

      {/* Input */}
      <div className="flex items-center gap-2 px-3 py-2.5 border-t border-white/10 flex-shrink-0">
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && sendMessage(input)}
          placeholder="Ask about your infrastructure..."
          className="flex-1 bg-transparent text-sm text-white px-2 py-1.5 focus:outline-none placeholder-slate-500"
          disabled={isTyping}
        />
        <button
          onClick={() => sendMessage(input)}
          disabled={!input.trim() || isTyping}
          className={cn(
            'p-2 rounded-lg transition-colors flex-shrink-0',
            input.trim() && !isTyping ? 'bg-brand-600 text-white hover:bg-brand-500' : 'bg-white/5 text-slate-600'
          )}
        >
          <Send className="w-4 h-4" />
        </button>
      </div>
    </div>
  )
}
