import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getElasticsearchIndices } from '@/lib/api'
export default function ElasticsearchMonitoringPage() {
  return <MiddlewareServicePage serviceType="elasticsearch" title="Elasticsearch" extraEndpoints={[{ label: 'Indices', fetcher: getElasticsearchIndices }]} />
}
