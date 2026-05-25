import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getDockerContainers } from '@/lib/api'
export default function DockerMonitoringPage() {
  return <MiddlewareServicePage serviceType="docker" title="Docker" extraEndpoints={[{ label: 'Containers', fetcher: getDockerContainers }]} />
}
