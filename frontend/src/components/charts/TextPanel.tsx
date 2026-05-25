interface TextPanelProps {
  content: string
  mode?: 'markdown' | 'html' | 'plain'
  className?: string
}

export default function TextPanel({ content, mode = 'plain', className = '' }: TextPanelProps) {
  if (mode === 'html') {
    return (
      <div
        className={`prose prose-invert prose-sm max-w-none ${className}`}
        dangerouslySetInnerHTML={{ __html: content }}
      />
    )
  }

  if (mode === 'markdown') {
    // Simple markdown rendering: headers, bold, italic, code, links, lists
    const lines = content.split('\n')
    const rendered = lines.map((line, i) => {
      // Headers
      if (line.startsWith('### ')) return <h3 key={i} className="text-sm font-bold text-white mt-3 mb-1">{line.slice(4)}</h3>
      if (line.startsWith('## ')) return <h2 key={i} className="text-base font-bold text-white mt-4 mb-1">{line.slice(3)}</h2>
      if (line.startsWith('# ')) return <h1 key={i} className="text-lg font-bold text-white mt-4 mb-2">{line.slice(2)}</h1>
      // Unordered list
      if (line.startsWith('- ') || line.startsWith('* ')) return <li key={i} className="text-xs text-slate-300 ml-4 list-disc">{formatInline(line.slice(2))}</li>
      // Horizontal rule
      if (line.match(/^---+$/)) return <hr key={i} className="border-white/10 my-2" />
      // Code block fence (simplified)
      if (line.startsWith('```')) return <div key={i} />
      // Empty line
      if (line.trim() === '') return <div key={i} className="h-2" />
      // Normal paragraph
      return <p key={i} className="text-xs text-slate-300 leading-relaxed">{formatInline(line)}</p>
    })
    return <div className={className}>{rendered}</div>
  }

  return <pre className={`text-xs text-slate-300 whitespace-pre-wrap font-mono ${className}`}>{content}</pre>
}

function formatInline(text: string): React.ReactNode {
  // Bold
  const parts = text.split(/(\*\*[^*]+\*\*|`[^`]+`)/)
  return parts.map((part, i) => {
    if (part.startsWith('**') && part.endsWith('**')) {
      return <strong key={i} className="text-white font-semibold">{part.slice(2, -2)}</strong>
    }
    if (part.startsWith('`') && part.endsWith('`')) {
      return <code key={i} className="px-1 py-0.5 bg-slate-800 rounded text-cyan-400 text-[10px] font-mono">{part.slice(1, -1)}</code>
    }
    return part
  })
}
