import { fetchJSON } from './_client'
import type { MiddlewareServiceStatus } from './types'

// Middleware (P5-P8)
export const getMiddlewareDashboard = () => fetchJSON<MiddlewareServiceStatus[]>('/middleware/dashboard')

// Generic middleware instance helpers
export const getMiddlewareInstances = (type_: string) => fetchJSON<any[]>(`/middleware/${type_}`)
export const createMiddlewareInstance = (type_: string, data: any) => fetchJSON<any>(`/middleware/${type_}`, { method: 'POST', body: JSON.stringify(data) })
export const deleteMiddlewareInstance = (type_: string, id: string) => fetchJSON<any>(`/middleware/${type_}/${id}`, { method: 'DELETE' })
export const getMiddlewareStatus = (type_: string, id: string) => fetchJSON<any>(`/middleware/${type_}/${id}/status`)
export const getMiddlewareHistory = (type_: string, id: string) => fetchJSON<any[]>(`/middleware/${type_}/${id}/performance-history`)

// Specialized endpoints
export const getNginxConfig = (id: string) => fetchJSON<any>(`/middleware/nginx/${id}/config`)
export const getApacheScoreboard = (id: string) => fetchJSON<any>(`/middleware/apache/${id}/scoreboard`)
export const getRabbitMQQueues = (id: string) => fetchJSON<any[]>(`/middleware/rabbitmq/${id}/queues`)
export const getKafkaTopics = (id: string) => fetchJSON<any[]>(`/middleware/kafka/${id}/topics`)
export const getElasticsearchIndices = (id: string) => fetchJSON<any[]>(`/middleware/elasticsearch/${id}/indices`)
export const getDockerContainers = (id: string) => fetchJSON<any[]>(`/middleware/docker/${id}/containers`)
export const getK8sNodes = (id: string) => fetchJSON<any[]>(`/middleware/kubernetes/${id}/nodes`)
export const getK8sPods = (id: string) => fetchJSON<any[]>(`/middleware/kubernetes/${id}/pods`)
export const getK8sDeployments = (id: string) => fetchJSON<any[]>(`/middleware/kubernetes/${id}/deployments`)
