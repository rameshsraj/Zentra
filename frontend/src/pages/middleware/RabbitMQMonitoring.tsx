import MiddlewareServicePage from '@/pages/MiddlewareServicePage'
import { getRabbitMQQueues } from '@/lib/api'
export default function RabbitMQMonitoringPage() {
  return <MiddlewareServicePage serviceType="rabbitmq" title="RabbitMQ" extraEndpoints={[{ label: 'Queues', fetcher: getRabbitMQQueues }]} />
}
