import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getKafkaTopics } from '@/lib/api'
export default function KafkaMonitoringPage() {
  return <MiddlewareServicePage serviceType="kafka" title="Kafka" extraEndpoints={[{ label: 'Topics', fetcher: getKafkaTopics }]} />
}
