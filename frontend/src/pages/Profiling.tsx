import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import FlameGraph from '@/components/charts/FlameGraph'
import { Cpu, Play, Clock, RefreshCw } from 'lucide-react'
import { cn } from '@/lib/utils'

interface ProfileJob {
  id: string
  target_id: string
  profile_type: string
  status: string
  duration_s: number
  pid_filter?: string
  triggered_at: string
  completed_at?: string
  error_message?: string
}

interface ProfileData {
  id: string
  job_id: string
  target_id: string
  profile_type: string
  collected_at: string
  duration_s: number
  sample_count: number
  flamegraph_data: any
}

export default function ProfilingPage() {
  const [source, setSource] = useState<'agent' | 'otel'>('agent')
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null)
  const [isTriggering, setIsTriggering] = useState(false)

  const { data: agentJobs, isLoading: agentLoading, refetch: refetchAgent } = useQuery({
    queryKey: ['profiling-jobs-agent'],
    queryFn: () => fetchJSON<ProfileJob[]>('/profiles'),
    enabled: source === 'agent',
    refetchInterval: 10000,
  })

  const { data: otelProfiles, isLoading: otelLoading, refetch: refetchOtel } = useQuery({
    queryKey: ['profiling-jobs-otel'],
    queryFn: () => fetchJSON<ProfileData[]>('/profiles/otel'),
    enabled: source === 'otel',
    refetchInterval: 30000,
  })

  const { data: profile, isLoading: profileLoading } = useQuery({
    queryKey: ['profile-detail', selectedJobId, source],
    queryFn: () => fetchJSON<ProfileData>(source === 'agent' ? `/profiles/${selectedJobId}` : `/profiles/otel/${selectedJobId}`),
    enabled: !!selectedJobId,
  })

  return (
    <div className="space-y-6">
      <PageHeader 
        title="Continuous Profiling" 
        subtitle="Agentless eBPF profiling and native OpenTelemetry profiles"
        onRefresh={() => source === 'agent' ? refetchAgent() : refetchOtel()}
      />

      {/* Source Toggle */}
      <div className="flex bg-white/5 border border-white/10 rounded-xl p-1 w-fit">
        <button
          onClick={() => { setSource('agent'); setSelectedJobId(null); }}
          className={cn(
            "px-4 py-2 text-xs font-bold rounded-lg transition-all",
            source === 'agent' ? "bg-brand-600 text-white shadow-lg" : "text-slate-500 hover:text-slate-300"
          )}
        >
          eBPF Agent
        </button>
        <button
          onClick={() => { setSource('otel'); setSelectedJobId(null); }}
          className={cn(
            "px-4 py-2 text-xs font-bold rounded-lg transition-all",
            source === 'otel' ? "bg-brand-600 text-white shadow-lg" : "text-slate-500 hover:text-slate-300"
          )}
        >
          OpenTelemetry
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Jobs List */}
        <div className="lg:col-span-1 space-y-4">
          <div className="bg-white/5 border border-white/10 rounded-xl p-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-bold text-white">
                {source === 'agent' ? 'Recent Profiling Jobs' : 'Discovered OTel Profiles'}
              </h3>
              {source === 'agent' && (
                <button 
                  onClick={() => setIsTriggering(true)}
                  disabled={isTriggering}
                  className="p-1.5 bg-brand-600 hover:bg-brand-700 rounded-lg text-white disabled:opacity-50 transition-colors"
                  title="Trigger new profile"
                >
                  <Play className="w-3.5 h-3.5" />
                </button>
              )}
            </div>

            {(agentLoading || otelLoading) && <LoadingSkeleton variant="card" count={3} />}
            
            <div className="space-y-2 max-h-[60vh] overflow-y-auto pr-1">
              {source === 'agent' ? (
                agentJobs?.map(job => (
                  <div 
                    key={job.id}
                    onClick={() => job.status === 'completed' && setSelectedJobId(job.id)}
                    className={cn(
                      "p-3 rounded-lg border transition-all cursor-pointer",
                      selectedJobId === job.id ? "bg-brand-500/10 border-brand-500/40" : "bg-white/5 border-white/10 hover:border-white/20",
                      job.status === 'running' && "animate-pulse"
                    )}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-xs font-bold text-white uppercase tracking-wider">{job.profile_type}</span>
                      <StatusBadge status={job.status} />
                    </div>
                    <div className="text-[10px] text-slate-500 flex items-center gap-1">
                      <Clock className="w-3 h-3" />
                      {new Date(job.triggered_at).toLocaleString()}
                    </div>
                  </div>
                ))
              ) : (
                otelProfiles?.map(p => (
                  <div 
                    key={p.id}
                    onClick={() => setSelectedJobId(p.id)}
                    className={cn(
                      "p-3 rounded-lg border transition-all cursor-pointer",
                      selectedJobId === p.id ? "bg-brand-500/10 border-brand-500/40" : "bg-white/5 border-white/10 hover:border-white/20"
                    )}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-xs font-bold text-white uppercase tracking-wider">{p.profile_type}</span>
                      <span className="text-[9px] px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 uppercase font-bold">OTEL</span>
                    </div>
                    <div className="text-[11px] text-white font-medium mb-1">{p.target_id}</div>
                    <div className="text-[10px] text-slate-500 flex items-center gap-1">
                      <Clock className="w-3 h-3" />
                      {new Date(p.collected_at).toLocaleString()}
                    </div>
                  </div>
                ))
              )}
              {((source === 'agent' && agentJobs?.length === 0) || (source === 'otel' && otelProfiles?.length === 0)) && (
                <div className="text-center py-8 text-xs text-slate-500">No profiles found for this source.</div>
              )}
            </div>
          </div>
        </div>

        {/* Profile Detail / Flamegraph */}
        <div className="lg:col-span-2">
          {!selectedJobId ? (
            <div className="bg-white/5 border border-white/10 rounded-xl h-full flex flex-col items-center justify-center p-12 text-center">
              <Cpu className="w-12 h-12 text-slate-700 mb-4" />
              <h3 className="text-lg font-bold text-white mb-2">No Profile Selected</h3>
              <p className="text-sm text-slate-400 max-w-xs mx-auto">
                Select a completed profiling job from the list to view its execution flamegraph.
              </p>
            </div>
          ) : profileLoading ? (
            <div className="bg-white/5 border border-white/10 rounded-xl p-6 h-full">
              <LoadingSkeleton variant="card" />
              <div className="mt-8 h-64 bg-white/5 rounded-lg animate-pulse" />
            </div>
          ) : profile ? (
            <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden flex flex-col h-full">
              <div className="px-4 py-3 border-b border-white/10 flex items-center justify-between bg-white/5">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-brand-500/20 rounded-lg">
                    <Cpu className="w-4 h-4 text-brand-400" />
                  </div>
                  <div>
                    <h3 className="text-sm font-bold text-white capitalize">{profile.profile_type} Profile</h3>
                    <div className="text-[10px] text-slate-500">
                      Collected {new Date(profile.collected_at).toLocaleString()} • {profile.duration_s}s
                    </div>
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-xs font-mono text-white">{profile.sample_count.toLocaleString()}</div>
                  <div className="text-[10px] text-slate-500 uppercase tracking-wider">Samples</div>
                </div>
              </div>
              
              <div className="p-4 flex-1 overflow-auto bg-slate-950">
                <FlameGraph data={profile.flamegraph_data} height={500} />
              </div>

              <div className="p-3 bg-black/20 border-t border-white/10 flex justify-between items-center text-[10px] text-slate-500">
                <span>Navigate using scroll and click to zoom (Coming Soon)</span>
                <span className="font-mono">ID: {profile.id.slice(0, 8)}…</span>
              </div>
            </div>
          ) : (
            <div className="bg-white/5 border border-white/10 rounded-xl p-12 text-center text-red-400">
              Failed to load profile data.
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function StatusBadge({ status }: { status: string }) {
  switch (status) {
    case 'completed':
      return <span className="text-[9px] px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 uppercase font-bold">Success</span>
    case 'running':
      return (
        <span className="text-[9px] px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 uppercase font-bold flex items-center gap-1">
          <RefreshCw className="w-2.5 h-2.5 animate-spin" />
          Running
        </span>
      )
    case 'failed':
      return <span className="text-[9px] px-1.5 py-0.5 rounded bg-red-500/10 text-red-400 border border-red-500/20 uppercase font-bold">Failed</span>
    default:
      return <span className="text-[9px] px-1.5 py-0.5 rounded bg-slate-500/10 text-slate-400 border border-slate-500/20 uppercase font-bold">{status}</span>
  }
}

