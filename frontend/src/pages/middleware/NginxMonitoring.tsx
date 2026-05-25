import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getNginxConfig } from '@/lib/api'
export default function NginxMonitoringPage() {
  return <MiddlewareServicePage serviceType="nginx" title="Nginx" extraEndpoints={[{ label: 'Config', fetcher: getNginxConfig }]} />
}
