import { useState, useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { huntSearch } from '../../lib/api'
import { Crosshair, Search, RefreshCw, Loader2 } from 'lucide-react'

const typeColor = (t: string) =>
  t === 'ids_alert' ? 'bg-red-600/20 text-red-400' : t === 'connection' ? 'bg-cyan-600/20 text-cyan-400' :
  t === 'dns' ? 'bg-green-600/20 text-green-400' : 'bg-gray-600/20 text-gray-400'

export default function ThreatHunting() {
  const qc = useQueryClient()
  const [query, setQuery] = useState('')
  const [searchTerm, setSearchTerm] = useState('')

  const { data, isLoading } = useQuery({
    queryKey: ['hunt', searchTerm],
    queryFn: () => huntSearch(searchTerm ? { q: searchTerm } : {}),
    enabled: true,
  })
  const refresh = useCallback(() => qc.invalidateQueries({ queryKey: ['hunt'] }), [qc])
  const results = data?.results || []

  const handleSearch = () => { setSearchTerm(query) }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-red-600/20"><Crosshair className="w-5 h-5 text-red-400" /></div>
          <div><h1 className="text-2xl font-bold text-white">Threat Hunting</h1><p className="text-sm text-gray-400">Unified search across security logs</p></div>
        </div>
        <button onClick={refresh} className="p-2 rounded-lg hover:bg-gray-700 text-gray-400"><RefreshCw className="w-4 h-4" /></button>
      </div>

      {/* Search */}
      <div className="flex gap-2">
        <div className="flex-1 relative">
          <Search className="w-4 h-4 absolute left-3 top-2.5 text-gray-500" />
          <input value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => e.key === 'Enter' && handleSearch()}
            placeholder="Search: alert, conn, dns, ip address..."
            className="w-full pl-10 pr-3 py-2 text-sm bg-gray-800 border border-gray-700 rounded-lg text-white" />
        </div>
        <button onClick={handleSearch} className="px-4 py-2 rounded-lg bg-red-600 hover:bg-red-700 text-white text-sm">Hunt</button>
      </div>

      {/* Results */}
      <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
        {isLoading ? <div className="p-8 text-center"><Loader2 className="w-5 h-5 animate-spin inline text-gray-500" /></div> : (
          <>
            <div className="px-4 py-2 border-b border-gray-700/50 text-xs text-gray-400">{results.length} results</div>
            <table className="w-full text-xs">
              <thead><tr className="text-gray-400 border-b border-gray-700/50 text-left">
                <th className="p-3">Type</th><th className="p-3">Time</th><th className="p-3">Source</th>
                <th className="p-3">Dest / Query</th><th className="p-3">Detail</th>
              </tr></thead>
              <tbody>{results.map((r: any, i: number) => (
                <tr key={r.id || i} className="border-b border-gray-700/30 hover:bg-gray-700/20">
                  <td className="p-3"><span className={`px-1.5 py-0.5 rounded text-[10px] ${typeColor(r.type)}`}>{r.type}</span></td>
                  <td className="p-3 text-gray-400 whitespace-nowrap">{new Date(r.timestamp).toLocaleString()}</td>
                  <td className="p-3 font-mono text-gray-300">{r.src_ip || '—'}</td>
                  <td className="p-3 font-mono text-gray-300">{r.dest_ip || r.query || '—'}</td>
                  <td className="p-3 text-gray-400">{r.severity || r.proto || r.rcode || '—'}</td>
                </tr>
              ))}</tbody>
            </table>
            {results.length === 0 && <p className="p-6 text-center text-gray-500 text-sm">No results found. Try a different search.</p>}
          </>
        )}
      </div>
    </div>
  )
}
