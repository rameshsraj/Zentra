import { fetchJSON } from './_client'

export interface Subscription {
  plan: string
  status: 'active' | 'trialling' | 'past_due' | 'cancelled'
  current_period_end: string | null
  features: {
    max_targets: number
    max_users: number
    max_dashboards: number
    max_alert_rules: number
    retention_days: number
  }
}

export function getSubscription(): Promise<Subscription> {
  return fetchJSON<Subscription>('/billing/subscription')
}

export function createCheckout(plan: string): Promise<{ checkout_url: string }> {
  return fetchJSON<{ checkout_url: string }>('/billing/checkout', {
    method: 'POST',
    body: JSON.stringify({ plan }),
  })
}

export function createBillingPortal(): Promise<{ portal_url: string }> {
  return fetchJSON<{ portal_url: string }>('/billing/portal', {
    method: 'POST',
  })
}
