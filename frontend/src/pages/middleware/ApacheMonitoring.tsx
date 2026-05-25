import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getApacheScoreboard } from '@/lib/api'
export default function ApacheMonitoringPage() {
  return <MiddlewareServicePage serviceType="apache" title="Apache" extraEndpoints={[{ label: 'Scoreboard', fetcher: getApacheScoreboard }]} />
}
