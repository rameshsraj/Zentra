import { useState } from 'react'
import { X, ChevronRight, ArrowLeft } from 'lucide-react'
import { cn } from '@/lib/utils'

export interface DrilldownNode {
  id: string
  label: string
  value?: string | number
  details?: Record<string, string | number | boolean>
  children?: DrilldownNode[]
}

interface DrilldownModalProps {
  open: boolean
  onClose: () => void
  title: string
  root: DrilldownNode[]
}

export default function DrilldownModal({ open, onClose, title, root }: DrilldownModalProps) {
  const [path, setPath] = useState<DrilldownNode[]>([])
  const currentNodes = path.length === 0 ? root : path[path.length - 1].children ?? []
  const currentTitle = path.length === 0 ? title : path[path.length - 1].label

  if (!open) return null

  const goBack = () => setPath((p) => p.slice(0, -1))
  const goInto = (node: DrilldownNode) => {
    if (node.children && node.children.length > 0) {
      setPath((p) => [...p, node])
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div
        className="bg-slate-900 border border-white/10 rounded-2xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center gap-3 p-4 border-b border-white/10">
          {path.length > 0 && (
            <button onClick={goBack} className="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors">
              <ArrowLeft className="w-4 h-4" />
            </button>
          )}
          <div className="flex-1">
            <h2 className="text-lg font-bold text-white">{currentTitle}</h2>
            {path.length > 0 && (
              <div className="flex items-center gap-1 text-[10px] text-slate-500 mt-0.5">
                <span>{title}</span>
                {path.map((p, i) => (
                  <span key={i} className="flex items-center gap-1">
                    <ChevronRight className="w-2.5 h-2.5" />
                    <span>{p.label}</span>
                  </span>
                ))}
              </div>
            )}
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-4 space-y-1 custom-scrollbar">
          {currentNodes.length === 0 ? (
            <div className="text-center text-slate-500 py-8">No data available</div>
          ) : (
            currentNodes.map((node) => (
              <div
                key={node.id}
                onClick={() => goInto(node)}
                className={cn(
                  'flex items-center gap-3 px-4 py-3 rounded-xl border transition-colors',
                  node.children && node.children.length > 0
                    ? 'bg-white/5 border-white/10 hover:bg-white/[0.08] cursor-pointer'
                    : 'bg-white/[0.02] border-white/5'
                )}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold text-white">{node.label}</div>
                  {node.details && (
                    <div className="flex flex-wrap gap-x-4 gap-y-0.5 mt-1">
                      {Object.entries(node.details).map(([k, v]) => (
                        <span key={k} className="text-[10px] text-slate-500">
                          <span className="text-slate-400">{k}:</span> {String(v)}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                {node.value !== undefined && (
                  <span className="text-sm font-mono font-bold text-brand-400">{node.value}</span>
                )}
                {node.children && node.children.length > 0 && (
                  <ChevronRight className="w-4 h-4 text-slate-600 flex-shrink-0" />
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
