import { fetchJSON } from './_client'
import type {
  ChatResponse, ChatMessage, AIInsight, RCAResult,
  McpToolDef, McpToolCallResponse, McpChatResponse,
  TraceSummary, TraceSpan,
  AppEvent, EventStats, DrilldownResult, SearchResult,
} from './types'

// AI / Chatbot
export const sendChatMessage = (message: string, sessionId: string) =>
  fetchJSON<ChatResponse>('/ai/chat', { method: 'POST', body: JSON.stringify({ message, session_id: sessionId }) })
export const getChatHistory = (sessionId: string) =>
  fetchJSON<ChatMessage[]>(`/ai/chat/history?session_id=${sessionId}`)
export const getAIInsights = () => fetchJSON<AIInsight[]>('/ai/insights')

// MCP (Model Context Protocol)
export const getMcpTools = () => fetchJSON<McpToolDef[]>('/mcp/tools')
export const callMcpTool = (tool: string, params: Record<string, unknown>) =>
  fetchJSON<McpToolCallResponse>('/mcp/call', { method: 'POST', body: JSON.stringify({ tool, params }) })
export const mcpChat = (message: string, sessionId: string, mode: string = 'mcp') =>
  fetchJSON<McpChatResponse>('/mcp/chat', { method: 'POST', body: JSON.stringify({ message, session_id: sessionId, mode }) })

// RCA
export const analyzeRCA = (logContent: string, source: string) =>
  fetchJSON<RCAResult>('/rca/analyze', { method: 'POST', body: JSON.stringify({ log_content: logContent, source }) })
export const getRCAHistory = () => fetchJSON<RCAResult[]>('/rca/history')

// P16: Traces API
export const searchTraces = (params?: { service?: string; operation?: string; limit?: number }) => {
  const qs = new URLSearchParams()
  if (params?.service) qs.set('service', params.service)
  if (params?.operation) qs.set('operation', params.operation)
  if (params?.limit) qs.set('limit', String(params.limit))
  return fetchJSON<{ traces: TraceSummary[]; count: number }>(`/traces?${qs}`)
}
export const getTrace = (traceId: string) =>
  fetchJSON<{ trace_id: string; spans: TraceSpan[]; span_count: number }>(`/traces/${traceId}`)
export const getTraceServices = () => fetchJSON<{ services: string[] }>('/traces/services')
export const getTraceOperations = (service?: string) => {
  const qs = service ? `?service=${service}` : ''
  return fetchJSON<{ operations: string[] }>(`/traces/operations${qs}`)
}
export const ingestTraces = (data: { spans: unknown[] }) =>
  fetchJSON<{ spans_ingested: number }>('/v1/traces', { method: 'POST', body: JSON.stringify(data) })

// Events
export const getRecentEvents = (limit = 50) =>
  fetchJSON<{ events: AppEvent[]; count: number; total_published: number }>(`/events?limit=${limit}`)
export const getEventStats = () => fetchJSON<EventStats>('/events/stats')

// Drilldown
export const getDrilldown = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params).toString()
  return fetchJSON<DrilldownResult>(`/drilldown?${qs}`)
}

// Search
export const globalSearch = (query: string) =>
  fetchJSON<SearchResult[]>(`/search?q=${encodeURIComponent(query)}`)
