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

export function getOnboardingChecklist(): Promise<OnboardingChecklist> {
  return fetchJSON<OnboardingChecklist>('/onboarding/checklist')
}
