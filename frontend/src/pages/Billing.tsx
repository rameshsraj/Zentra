import { useQuery, useMutation } from '@tanstack/react-query'
import { getSubscription, createCheckout, createBillingPortal, type Subscription } from '@/lib/api/billing'
import PageHeader from '@/components/PageHeader'
import { CreditCard, ExternalLink, CheckCircle2, AlertTriangle, XCircle } from 'lucide-react'

const PLANS = [
  {
    name: 'Starter',
    key: 'starter',
    price: '$29/mo',
    limits: { targets: 10, users: 5, dashboards: 10, alerts: 50, retention: '30 days' },
  },
  {
    name: 'Professional',
    key: 'professional',
    price: '$99/mo',
    limits: { targets: 100, users: 20, dashboards: 50, alerts: 200, retention: '90 days' },
    popular: true,
  },
  {
    name: 'Enterprise',
    key: 'enterprise',
    price: 'Custom',
    limits: { targets: 500, users: 50, dashboards: 200, alerts: 1000, retention: '365 days' },
  },
]

const STATUS_BADGE: Record<string, { icon: typeof CheckCircle2; color: string; label: string }> = {
  active: { icon: CheckCircle2, color: 'text-green-400 bg-green-500/10', label: 'Active' },
  trialling: { icon: CheckCircle2, color: 'text-blue-400 bg-blue-500/10', label: 'Trial' },
  past_due: { icon: AlertTriangle, color: 'text-yellow-400 bg-yellow-500/10', label: 'Past Due' },
  cancelled: { icon: XCircle, color: 'text-red-400 bg-red-500/10', label: 'Cancelled' },
}

export default function BillingPage() {
  const { data: sub, isLoading } = useQuery({
    queryKey: ['billing-subscription'],
    queryFn: getSubscription,
  })

  const checkoutMut = useMutation({
    mutationFn: createCheckout,
    onSuccess: (data) => { window.location.href = data.checkout_url },
  })

  const portalMut = useMutation({
    mutationFn: createBillingPortal,
    onSuccess: (data) => { window.location.href = data.portal_url },
  })

  const badge = sub ? STATUS_BADGE[sub.status] ?? STATUS_BADGE.active : null

  return (
    <div className="space-y-8">
      <PageHeader title="Billing & Plan" description="Manage your subscription and billing." icon={CreditCard} />

      {/* Past Due Banner */}
      {sub?.status === 'past_due' && (
        <div className="rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-4 flex items-center gap-3">
          <AlertTriangle className="h-5 w-5 text-yellow-400 shrink-0" />
          <div className="flex-1">
            <p className="text-sm font-medium text-yellow-400">Payment failed</p>
            <p className="text-xs text-muted-foreground">
              Update your payment method to avoid losing access.
            </p>
          </div>
          <button
            onClick={() => portalMut.mutate()}
            className="rounded-lg bg-yellow-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-yellow-700"
          >
            Fix Payment
          </button>
        </div>
      )}

      {/* Current Plan */}
      {!isLoading && sub && (
        <div className="rounded-xl border border-border bg-card p-6 space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-lg font-semibold capitalize">{sub.plan} Plan</h3>
              {sub.current_period_end && (
                <p className="text-xs text-muted-foreground">
                  Renews {new Date(sub.current_period_end).toLocaleDateString()}
                </p>
              )}
            </div>
            {badge && (
              <span className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-medium ${badge.color}`}>
                <badge.icon className="h-3.5 w-3.5" />
                {badge.label}
              </span>
            )}
          </div>

          {/* Usage gauges */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
            <UsageGauge label="Targets" max={sub.features.max_targets} />
            <UsageGauge label="Users" max={sub.features.max_users} />
            <UsageGauge label="Dashboards" max={sub.features.max_dashboards} />
            <UsageGauge label="Alert Rules" max={sub.features.max_alert_rules} />
          </div>

          <div className="flex gap-3">
            {sub.plan !== 'enterprise' && (
              <button
                onClick={() => checkoutMut.mutate('professional')}
                disabled={checkoutMut.isPending}
                className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
              >
                Upgrade Plan
              </button>
            )}
            <button
              onClick={() => portalMut.mutate()}
              disabled={portalMut.isPending}
              className="flex items-center gap-2 rounded-lg bg-zinc-700 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-600 disabled:opacity-50"
            >
              <ExternalLink className="h-4 w-4" /> Manage Billing
            </button>
          </div>
        </div>
      )}

      {/* Plan Comparison */}
      <div>
        <h3 className="text-lg font-semibold mb-4">Compare Plans</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {PLANS.map((p) => (
            <div
              key={p.key}
              className={`rounded-xl border p-6 space-y-4 ${
                p.popular ? 'border-blue-500 bg-blue-500/5' : 'border-border bg-card'
              }`}
            >
              {p.popular && (
                <span className="inline-block rounded-full bg-blue-600 px-2.5 py-0.5 text-xs font-medium text-white">
                  Most Popular
                </span>
              )}
              <div>
                <h4 className="text-xl font-bold">{p.name}</h4>
                <p className="text-2xl font-bold mt-1">{p.price}</p>
              </div>
              <ul className="space-y-2 text-sm text-muted-foreground">
                <li>Up to {p.limits.targets} targets</li>
                <li>Up to {p.limits.users} users</li>
                <li>{p.limits.dashboards} dashboards</li>
                <li>{p.limits.alerts} alert rules</li>
                <li>{p.limits.retention} retention</li>
              </ul>
              {sub?.plan !== p.key && p.key !== 'enterprise' && (
                <button
                  onClick={() => checkoutMut.mutate(p.key)}
                  disabled={checkoutMut.isPending}
                  className="w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
                >
                  {sub && PLANS.findIndex((x) => x.key === sub.plan) < PLANS.findIndex((x) => x.key === p.key) ? 'Upgrade' : 'Switch'}
                </button>
              )}
              {p.key === 'enterprise' && (
                <a
                  href="mailto:sales@zentra.dev"
                  className="block w-full text-center rounded-lg border border-border px-4 py-2 text-sm font-medium hover:bg-muted transition-colors"
                >
                  Contact Sales
                </a>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function UsageGauge({ label, max }: { label: string; max: number }) {
  // Placeholder — actual usage would come from separate API
  const used = 0
  const pct = max > 0 ? Math.min((used / max) * 100, 100) : 0
  return (
    <div className="space-y-1.5">
      <div className="flex justify-between text-xs">
        <span className="text-muted-foreground">{label}</span>
        <span>{used}/{max}</span>
      </div>
      <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${pct > 80 ? 'bg-red-500' : 'bg-blue-500'}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  )
}
