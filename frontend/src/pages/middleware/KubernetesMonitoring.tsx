import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getK8sNodes, getK8sPods } from '@/lib/api'
export default function KubernetesMonitoringPage() {
  return <MiddlewareServicePage serviceType="kubernetes" title="Kubernetes" extraEndpoints={[{ label: 'Nodes', fetcher: getK8sNodes }, { label: 'Pods', fetcher: getK8sPods }]} />
}
