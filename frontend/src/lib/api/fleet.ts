import { fetchJSON } from './_client'

export interface Agent {
  agent_id: string
  datacenter_name: string
  description: string | null
  region_label: string | null
  agent_version: string | null
  status: 'active' | 'stale' | 'offline' | 'deregistered'
  last_seen: string | null
  ip_address: string | null
  os_name: string | null
  active_target_count: number
  created_at: string
}

export interface RegisterAgentResponse {
  agent_id: string
  api_key: string
  message: string
}

export interface OnboardingChecklist {
  email_verified: boolean
  agent_registered: boolean
  target_added: boolean
  metric_received: boolean
  dashboard_viewed: boolean
  all_complete: boolean
}

export function listAgents(): Promise<Agent[]> {
  return fetchJSON<Agent[]>('/agents')
}

export function registerAgent(body: {
  datacenter_name: string
  description?: string
  region_label?: string
}): Promise<RegisterAgentResponse> {
  return fetchJSON<RegisterAgentResponse>('/agents/register', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function deregisterAgent(agentId: string): Promise<{ message: string }> {
  return fetchJSON<{ message: string }>(`/agents/${agentId}`, {
    method: 'DELETE',
  })
}

export interface AgentKey {
  id: string
  name: string
  key_prefix: string
  enabled: boolean
  created_at: string | null
  rate_limit_rpm: number | null
  agent_id: string | null
  datacenter_name: string | null
  agent_status: string | null
  last_seen: string | null
}

export interface CreatedAgentKey extends AgentKey {
  api_key: string
  warning: string
}

export function listAgentKeys(): Promise<AgentKey[]> {
  return fetchJSON<AgentKey[]>('/agent-keys')
}

export function createAgentKey(body: { name: string; description?: string }): Promise<CreatedAgentKey> {
  return fetchJSON<CreatedAgentKey>('/agent-keys', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function revokeAgentKey(keyId: string): Promise<{ revoked: boolean; id: string }> {
  return fetchJSON<{ revoked: boolean; id: string }>(`/agent-keys/${keyId}`, {
    method: 'DELETE',
  })
}

export function getOnboardingChecklist(): Promise<OnboardingChecklist> {
  return fetchJSON<OnboardingChecklist>('/onboarding/checklist')
}

export interface AgentLogLine {
  timestamp: string
  level: string
  target: string
  message: string
}

export function getAgentLogsByDc(dcName: string): Promise<{ lines: AgentLogLine[] }> {
  return fetchJSON<{ lines: AgentLogLine[] }>(`/agents/by-dc/${encodeURIComponent(dcName)}/logs`)
}

export interface DiscoveredHost {
  id: string
  ip_address: string
  hostname: string | null
  open_ports: number[]
  os_hint: string | null
  first_seen: string
  last_seen: string
  opted_in: boolean
  ignored: boolean
  target_id: string | null
}

export function listDiscoveredHosts(agentId: string): Promise<DiscoveredHost[]> {
  return fetchJSON<DiscoveredHost[]>(`/agents/${agentId}/discovered`)
}

export function optInHost(
  agentId: string,
  hostId: string,
  body: { opted_in?: boolean; ignored?: boolean }
): Promise<{ updated: boolean; id: string }> {
  return fetchJSON<{ updated: boolean; id: string }>(
    `/agents/${agentId}/discovered/${hostId}`,
    { method: 'PATCH', body: JSON.stringify(body) }
  )
}

export interface DiscoveredComponent {
  id: string
  component_type: string
  component_name: string
  address: string
  port: number | null
  version: string | null
  extra: Record<string, unknown>
  confirmed: boolean
  target_id: string | null
  first_seen: string
  last_seen: string
  agent_id: string
  agent_status: string
}

export interface ComponentsResponse {
  components: DiscoveredComponent[]
  datacenter: string
}

export function getComponentsByDc(dcName: string): Promise<ComponentsResponse> {
  return fetchJSON<ComponentsResponse>(`/agents/by-dc/${encodeURIComponent(dcName)}/components`)
}

export interface ConfirmedTarget {
  target_id: string
  name: string
  host: string
  subtype: string
  component_id: string
}

export function confirmComponents(body: {
  component_ids: string[]
  datacenter_name: string
}): Promise<{ created: number; targets: ConfirmedTarget[] }> {
  return fetchJSON<{ created: number; targets: ConfirmedTarget[] }>(
    '/agents/components/confirm',
    { method: 'POST', body: JSON.stringify(body) }
  )
}
