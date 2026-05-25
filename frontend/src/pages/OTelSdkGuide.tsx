import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import { Copy, CheckCircle, ExternalLink, Terminal, Code2, BookOpen } from 'lucide-react'

interface SdkSnippet {
  language: string
  framework?: string
  install: string
  command?: string
  env_vars: Record<string, string>
}

const languages = [
  { id: 'java', label: 'Java', framework: 'spring-boot' },
  { id: 'python', label: 'Python', framework: 'flask' },
  { id: 'go', label: 'Go', framework: 'net-http' },
  { id: 'nodejs', label: 'Node.js', framework: 'express' },
  { id: 'dotnet', label: '.NET', framework: 'aspnet' },
  { id: 'rust', label: 'Rust', framework: 'axum' },
]

export default function OTelSdkGuide() {
  const [lang, setLang] = useState('java')
  const selected = languages.find(l => l.id === lang) || languages[0]

  const { data: snippet } = useQuery({
    queryKey: ['sdk-snippet', lang],
    queryFn: () => fetchJSON<SdkSnippet>(`/otel/sdk-snippets?language=${lang}&framework=${selected.framework}`),
  })

  return (
    <div className="space-y-6">
      <PageHeader
        title="SDK Instrumentation Guide"
        subtitle="Instrument your services with OpenTelemetry SDKs — send traces, metrics, and logs to Zentra"
      />

      {/* Java highlight banner */}
      <div className="bg-gradient-to-r from-orange-900/30 to-amber-900/20 border border-orange-700/40 rounded-xl p-5">
        <div className="flex items-start gap-3">
          <Terminal className="w-5 h-5 text-orange-400 mt-0.5 flex-shrink-0" />
          <div>
            <h3 className="text-sm font-semibold text-orange-300 mb-1">Java Auto-Instrumentation — Zero Code Changes</h3>
            <p className="text-xs text-slate-300 mb-3">
              The OpenTelemetry Java agent automatically instruments Spring Boot, JDBC, Kafka, gRPC, and HTTP clients.
              Just attach the JAR — no code changes required.
            </p>
            <pre className="bg-slate-950/80 rounded-lg p-3 text-xs text-amber-300 font-mono overflow-auto">
{`java -javaagent:opentelemetry-javaagent.jar \\
  -Dotel.exporter.otlp.endpoint=http://<zentra-host>:3001 \\
  -Dotel.exporter.otlp.protocol=http/json \\
  -Dotel.service.name=my-service \\
  -jar my-app.jar`}
            </pre>
          </div>
        </div>
      </div>

      {/* Language tabs */}
      <div className="flex gap-1 bg-slate-800/60 border border-slate-700 rounded-xl p-1">
        {languages.map(l => (
          <button
            key={l.id}
            onClick={() => setLang(l.id)}
            className={cn(
              'flex-1 px-3 py-2 rounded-lg text-sm font-medium transition-all',
              lang === l.id
                ? 'bg-cyan-600 text-white shadow-lg'
                : 'text-slate-400 hover:text-white hover:bg-slate-700/50'
            )}
          >
            {l.label}
          </button>
        ))}
      </div>

      {/* Snippet content */}
      {snippet && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Install + Command */}
          <div className="space-y-4">
            <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
              <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-3">
                <Code2 className="w-4 h-4 text-emerald-400" />
                Installation
              </h3>
              <CodeBlock code={snippet.install} />
            </div>

            {snippet.command && (
              <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
                <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-3">
                  <Terminal className="w-4 h-4 text-cyan-400" />
                  Run Command
                </h3>
                <CodeBlock code={snippet.command} />
              </div>
            )}
          </div>

          {/* Env vars + Links */}
          <div className="space-y-4">
            <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
              <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-3">
                <BookOpen className="w-4 h-4 text-amber-400" />
                Environment Variables
              </h3>
              <div className="space-y-2">
                {Object.entries(snippet.env_vars).map(([key, val]) => (
                  <div key={key} className="bg-slate-900 rounded-lg p-3">
                    <div className="text-xs text-cyan-300 font-mono mb-1">{key}</div>
                    <div className="text-xs text-slate-300 font-mono">{val}</div>
                  </div>
                ))}
              </div>
            </div>

            <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
              <h3 className="text-sm font-semibold text-white mb-3">Next Steps</h3>
              <div className="space-y-2 text-xs">
                <a href="/otel-config" className="flex items-center gap-2 text-cyan-400 hover:text-cyan-300 transition">
                  <ExternalLink className="w-3 h-3" />
                  Configure the Collector pipeline →
                </a>
                <a href="/otel-dashboard" className="flex items-center gap-2 text-cyan-400 hover:text-cyan-300 transition">
                  <ExternalLink className="w-3 h-3" />
                  View telemetry health dashboard →
                </a>
                <a href="/otel-resources" className="flex items-center gap-2 text-cyan-400 hover:text-cyan-300 transition">
                  <ExternalLink className="w-3 h-3" />
                  Verify your service appears in resources →
                </a>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function CodeBlock({ code }: { code: string }) {
  const [copied, setCopied] = useState(false)
  const handleCopy = () => {
    navigator.clipboard.writeText(code)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="relative group">
      <pre className="bg-slate-950 border border-slate-700 rounded-lg p-4 text-xs text-green-300 font-mono overflow-auto whitespace-pre-wrap">
        {code}
      </pre>
      <button
        onClick={handleCopy}
        className="absolute top-2 right-2 p-1.5 rounded bg-slate-700/80 text-slate-400 hover:text-white opacity-0 group-hover:opacity-100 transition"
      >
        {copied ? <CheckCircle className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
      </button>
    </div>
  )
}
