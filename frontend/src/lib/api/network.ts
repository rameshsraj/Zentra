import { fetchJSON } from './_client'
import type {
  NetworkCapture, NetworkConnection, NetworkHost, TrafficDataPoint, NetworkStats,
  PaginatedConnections, PcapExportStatus, PcapImportResult,
  NetworkFavoriteItem, BlacklistEntryType, NotificationRule, NotificationLogEntry,
  DeepTrafficSnapshot, ServiceEntry, IcmpEntry, ThreatListStatus, DiscoveredDevice,
  NetworkInterface, ProtocolStat, SocketStats, ContainerNetInfo,
} from './types'

// Network Capture
export const getNetworkCapture = () => fetchJSON<NetworkCapture>('/network/capture')
export const getNetworkConnections = () => fetchJSON<NetworkConnection[]>('/network/connections')
export const getNetworkHosts = () => fetchJSON<NetworkHost[]>('/network/hosts')
export const getNetworkTraffic = (range?: string) =>
  fetchJSON<TrafficDataPoint[]>(`/network/traffic${range ? `?range=${range}` : ''}`)
export const startCapture = (iface: string) =>
  fetchJSON<{ message: string }>('/network/capture/control', {
    method: 'POST', body: JSON.stringify({ action: 'start', interface: iface })
  })
export const stopCapture = () =>
  fetchJSON<{ message: string }>('/network/capture/control', {
    method: 'POST', body: JSON.stringify({ action: 'stop' })
  })

// S4: Real Capture Stats API
export const getNetworkStats = () => fetchJSON<NetworkStats>('/network/stats')
export const getTrafficChart = (range?: string, repr?: string) => {
  const params = new URLSearchParams()
  if (range) params.set('range', range)
  if (repr) params.set('repr', repr)
  return fetchJSON<TrafficDataPoint[]>(`/network/traffic/chart?${params}`)
}
export const getProtocolBreakdown = () =>
  fetchJSON<{ name: string; bytes: number; packets: number; percentage: number }[]>('/network/traffic/protocols')
export const getCountryStats = () =>
  fetchJSON<{ code: string; name: string; bytes_in: number; bytes_out: number; connections: number }[]>('/network/traffic/countries')
export const getPaginatedConnections = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params)
  return fetchJSON<PaginatedConnections>(`/network/connections/paginated?${qs}`)
}

// S5: PCAP Import & Export
export const startPcapExport = (fileName?: string) =>
  fetchJSON<PcapExportStatus>('/network/capture/export', {
    method: 'POST', body: JSON.stringify({ file_name: fileName })
  })
export const getPcapExportStatus = () => fetchJSON<PcapExportStatus>('/network/capture/export/status')
export const importPcap = () =>
  fetchJSON<PcapImportResult>('/network/capture/import', { method: 'POST' })

// S7: Favorites System
export const listNetworkFavorites = () => fetchJSON<NetworkFavoriteItem[]>('/network/favorites/list')
export const addNetworkFavoriteItem = (type_: string, value: Record<string, unknown>) =>
  fetchJSON<NetworkFavoriteItem>('/network/favorites/add', {
    method: 'POST', body: JSON.stringify({ favorite_type: type_, value })
  })
export const deleteNetworkFavoriteItem = (id: string) =>
  fetchJSON<{ deleted: boolean; id: string }>(`/network/favorites/remove/${id}`, { method: 'DELETE' })

// S8: IP Blacklist Management
export const listBlacklist = () => fetchJSON<BlacklistEntryType[]>('/network/blacklist')
export const addBlacklistIP = (ip: string, reason?: string) =>
  fetchJSON<BlacklistEntryType>('/network/blacklist/add', {
    method: 'POST', body: JSON.stringify({ ip, reason })
  })
export const removeBlacklistIP = (ip: string) =>
  fetchJSON<{ removed: boolean; ip: string }>(`/network/blacklist/remove/${ip}`, { method: 'DELETE' })
export const importBlacklistFile = () =>
  fetchJSON<{ status: string; entries_added: number }>('/network/blacklist/import', { method: 'POST' })

// S9: Network Notifications
export const getNotificationRules = () => fetchJSON<NotificationRule[]>('/network/notification-rules')
export const createNotificationRule = (rule_type: string, config: Record<string, unknown>) =>
  fetchJSON<NotificationRule>('/network/notification-rules', {
    method: 'POST', body: JSON.stringify({ rule_type, config })
  })
export const deleteNotificationRule = (id: string) =>
  fetchJSON<{ deleted: boolean; id: string }>(`/network/notification-rules/${id}`, { method: 'DELETE' })
export const getNotificationLog = () => fetchJSON<NotificationLogEntry[]>('/network/notification-log')

// Network Device Discovery & Subnet Scan
export const discoverDevices = (subnet: string) =>
  fetchJSON<DiscoveredDevice[]>('/network/discovery/scan', { method: 'POST', body: JSON.stringify({ subnet }) })
export const getDiscoveredDevices = () => fetchJSON<DiscoveredDevice[]>('/network/discovery/devices')
export const traceroute = (host: string, protocol?: string, options?: Record<string, unknown>) =>
  fetchJSON<any>('/network/traceroute', {
    method: 'POST',
    body: JSON.stringify({ host, protocol, ...options }),
  })

// Network Deep Analysis (Sniffnet-inspired)
export const getDeepTrafficSnapshot = (topN?: number) =>
  fetchJSON<DeepTrafficSnapshot>(`/network/deep/snapshot${topN ? `?top_n=${topN}` : ''}`)
export const getServiceBreakdown = () => fetchJSON<ServiceEntry[]>('/network/deep/services')
export const getIcmpBreakdown = () => fetchJSON<IcmpEntry[]>('/network/deep/icmp')
export const getThreatListStatus = () => fetchJSON<ThreatListStatus>('/network/deep/threat-status')
export const addNetworkFavorite = (ip: string) =>
  fetchJSON<{ added: string }>('/network/deep/favorites', { method: 'POST', body: JSON.stringify({ ip }) })
export const removeNetworkFavorite = (ip: string) =>
  fetchJSON<{ removed: string }>(`/network/deep/favorites/${encodeURIComponent(ip)}`, { method: 'DELETE' })

// P3: Network Interfaces, Protocol Stats, Socket Stats
export const getNetworkInterfaces = () => fetchJSON<NetworkInterface[]>('/network/interfaces')
export const getProtocolStats = () => fetchJSON<ProtocolStat[]>('/network/protocol-stats')
export const getSocketStats = () => fetchJSON<SocketStats>('/network/socket-stats')

// P17: Container Awareness
export const getNetworkContainers = () => fetchJSON<ContainerNetInfo[]>('/network/containers')
export const getFilteredConnections = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params)
  return fetchJSON<any[]>(`/network/connections/filtered?${qs}`)
}

// Network Topology (V5 Prompt 20)
export const getNetworkTopology = () => fetchJSON<any>('/network/topology')
export const topologyAutoDiscover = () => fetchJSON<any>('/network/topology/auto-discover')
export const getTopologyNeighbors = (nodeId: string) => fetchJSON<any>(`/network/topology/${nodeId}/neighbors`)
export const getTopologyShortestPath = (src: string, dest: string) => fetchJSON<any>(`/network/topology/paths/${src}/${dest}`)

// Compatibility aliases used by Dashboard.tsx
export const getNetworkProtocols = () => fetchJSON<{ name: string; count: number }[]>('/network/protocols')
export const getNetworkBandwidth = () => fetchJSON<TrafficDataPoint[]>('/network/traffic?range=1h')

// Network Device Discovery — POST endpoints
export interface SubnetScanResult {
  cidr: string; total_scanned: number; devices_found: number; scan_duration_ms: number;
  devices: { ip: string; hostname: string | null; mac: string | null; device_type: string; vendor: string | null; snmp_reachable: boolean; ssh_reachable: boolean; ping_reachable: boolean; sys_descr: string | null; sys_name: string | null; uptime: string | null; open_ports: number[]; response_time_ms: number }[]
}
export const scanSubnet = (cidr: string, snmp_community?: string, snmp_version?: string, timeout_ms?: number) =>
  fetchJSON<SubnetScanResult>('/network/scan', {
    method: 'POST',
    body: JSON.stringify({ cidr, snmp_community, snmp_version, timeout_ms }),
  })
export const testNetworkDevice = (host: string, protocol: string, opts?: { port?: number; snmp_community?: string; snmp_version?: string; username?: string; password?: string }) =>
  fetchJSON<{ success: boolean; host: string; protocol: string; response_time_ms: number; sys_descr: string | null; sys_name: string | null; device_type: string | null; vendor: string | null; message: string }>('/network/test-device', {
    method: 'POST',
    body: JSON.stringify({ host, protocol, port: opts?.port, snmp_community: opts?.snmp_community, snmp_version: opts?.snmp_version, username: opts?.username, password: opts?.password }),
  })

// Network Devices — registered targets with live SNMP status
export interface NetworkDeviceStatus {
  target_id: string; name: string; host: string; port: number; subtype: string;
  description: string | null; status: string; snmp_reachable: boolean;
  sys_descr: string | null; sys_name: string | null; uptime: string | null;
  vendor: string | null; device_type: string | null; response_time_ms: number | null;
  last_check_at: string | null; created_at: string;
}
export const getNetworkDevices = () => fetchJSON<NetworkDeviceStatus[]>('/network/devices')

// Detailed SNMP metrics for a single device
export interface DeviceInterface {
  index: number; name: string; descr: string; if_type: string;
  speed_mbps: number; admin_status: string; oper_status: string;
  in_octets: number; out_octets: number; in_errors: number; out_errors: number;
  mtu: number; mac_address: string;
}
export interface NetworkDeviceDetail {
  target_id: string; name: string; host: string; port: number; subtype: string;
  snmp_reachable: boolean;
  sys_descr: string | null; sys_name: string | null;
  sys_location: string | null; sys_contact: string | null;
  uptime: string | null; vendor: string | null; device_type: string | null;
  response_time_ms: number | null;
  interfaces: DeviceInterface[];
  cpu_usage_pct: number | null;
  mem_total_kb: number | null;
  mem_used_kb: number | null;
  temperature_c: number | null;
}
export const getNetworkDeviceDetail = (id: string) => fetchJSON<NetworkDeviceDetail>(`/network/devices/${id}`)

// Compatibility alias used by NetworkMiniWidget
export const getNetworkServiceBreakdown = () => fetchJSON<{ service: string; count: number; percentage: number }[]>('/network/deep/services')

// Stubs for Network.tsx (adapters/programs not yet in API)
export const getNetworkAdapters = () => fetchJSON<{ name: string; ip: string; mac: string }[]>('/network/interfaces/list')
export const getNetworkPrograms = () => fetchJSON<{ pid: number; name: string; connections: number }[]>('/network/programs')

// Remaining compatibility stubs
export const getCountryBreakdown = () => fetchJSON<{ code: string; name: string; bytes: number; connections: number }[]>('/network/deep/countries')
export const stopPcapExport = () => fetchJSON<{ stopped: boolean }>('/network/capture/export/stop', { method: 'POST' })
export interface ServiceBreakdownEntry { service: string; count: number; percentage: number }
