import { lazy, Suspense } from 'react'
import { Routes, Route, Navigate } from 'react-router-dom'
import { AuthProvider } from './contexts/AuthContext'
import RequireAuth from './components/RequireAuth'
import Layout from './components/Layout'

// Auth
const LoginPage = lazy(() => import('./pages/LoginPage'))
const ForgotPasswordPage = lazy(() => import('./pages/auth/ForgotPassword'))
const ResetPasswordPage = lazy(() => import('./pages/auth/ResetPassword'))

// Command Center
const DashboardPage = lazy(() => import('./pages/Dashboard'))

// Infrastructure
const InfrastructureDashboardPage = lazy(() => import('./pages/InfrastructureDashboard'))
const ServersPage = lazy(() => import('./pages/Servers'))
const StorageArraysPage = lazy(() => import('./pages/StorageArrays'))
const NetworkPage = lazy(() => import('./pages/Network'))

// Database
const DatabaseDashboardPage = lazy(() => import('./pages/DatabaseDashboard'))
const OracleMonitoringPage = lazy(() => import('./pages/OracleMonitoring'))
const OracleMetricEditorPage = lazy(() => import('./pages/OracleMetricEditor'))

// Cilium / eBPF
const CiliumMonitoringPage = lazy(() => import('./pages/CiliumMonitoring'))

// Middleware
const MiddlewareMonitoringPage = lazy(() => import('./pages/MiddlewareMonitoring'))
const NginxMonitoringPage = lazy(() => import('./pages/middleware/NginxMonitoring'))
const ApacheMonitoringPage = lazy(() => import('./pages/middleware/ApacheMonitoring'))
const TomcatMonitoringPage = lazy(() => import('./pages/middleware/TomcatMonitoring'))
const HAProxyMonitoringPage = lazy(() => import('./pages/middleware/HAProxyMonitoring'))
const ElasticsearchMonitoringPage = lazy(() => import('./pages/middleware/ElasticsearchMonitoring'))
const RedisMonitoringPage = lazy(() => import('./pages/middleware/RedisMonitoring'))
const MemcachedMonitoringPage = lazy(() => import('./pages/middleware/MemcachedMonitoring'))
const RabbitMQMonitoringPage = lazy(() => import('./pages/middleware/RabbitMQMonitoring'))
const KafkaMonitoringPage = lazy(() => import('./pages/middleware/KafkaMonitoring'))
const DockerMonitoringPage = lazy(() => import('./pages/middleware/DockerMonitoring'))
const KubernetesMonitoringPage = lazy(() => import('./pages/middleware/KubernetesMonitoring'))
const PhpFpmMonitoringPage = lazy(() => import('./pages/middleware/PhpFpmMonitoring'))

// Security
const SecurityDashboardPage = lazy(() => import('./pages/security/SecurityDashboard'))
const ThreatHuntingPage = lazy(() => import('./pages/security/ThreatHunting'))
const FirewallMonitoringPage = lazy(() => import('./pages/security/FirewallMonitoring'))

// Logs
const LogExplorerPage = lazy(() => import('./pages/LogExplorer'))
const LogSourcesPage = lazy(() => import('./pages/LogSources'))
const LogAlertsPage = lazy(() => import('./pages/LogAlerts'))
const LogAnalyticsPage = lazy(() => import('./pages/LogAnalytics'))

// Alerts & Events
const AlertsPage = lazy(() => import('./pages/Alerts'))
const AlertRulesPage = lazy(() => import('./pages/AlertRules'))
const NotificationChannelsPage = lazy(() => import('./pages/NotificationChannels'))

// Admin
const UserManagementPage = lazy(() => import('./pages/admin/UserManagement'))
const UserGroupManagementPage = lazy(() => import('./pages/admin/UserGroupManagement'))
const RoleManagementPage = lazy(() => import('./pages/admin/RoleManagement'))
const UserProfilePage = lazy(() => import('./pages/admin/UserProfile'))
const OrganizationManagementPage = lazy(() => import('./pages/admin/OrganizationManagement'))

// AI & Analytics
const AIInsightsPage = lazy(() => import('./pages/AIInsights'))
const PredictiveAlertsPage = lazy(() => import('./pages/PredictiveAlerts'))
const RCAPage = lazy(() => import('./pages/RCA'))
const RunbooksPage = lazy(() => import('./pages/Runbooks'))
const LLMObservabilityPage = lazy(() => import('./pages/LLMObservability'))
const ProfilingPage = lazy(() => import('./pages/Profiling'))

// OpenTelemetry
const TracesPage = lazy(() => import('./pages/Traces'))
const OTelCollectorConfigPage = lazy(() => import('./pages/OTelCollectorConfig'))
const OTelSdkGuidePage = lazy(() => import('./pages/OTelSdkGuide'))
const OTelResourcesPage = lazy(() => import('./pages/OTelResources'))
const OTelDashboardPage = lazy(() => import('./pages/OTelDashboard'))
const OTelConnectorsPage = lazy(() => import('./pages/OTelConnectors'))
const OTelPipelinePage = lazy(() => import('./pages/OTelPipeline'))
const OTelSpanMetricsPage = lazy(() => import('./pages/OTelSpanMetrics'))
const OTelAlertRulesPage = lazy(() => import('./pages/OTelAlertRules'))
const OTelSecurityPage = lazy(() => import('./pages/OTelSecurity'))

// Cloud Monitoring (P10)
const CloudMonitoringPage = lazy(() => import('./pages/CloudMonitoring'))

// Storage Deep-Dive (P11)
const StorageDeepDivePage = lazy(() => import('./pages/StorageDeepDive'))

// Fleet
const FleetPage = lazy(() => import('./pages/Fleet'))
const AgentInstallWizardPage = lazy(() => import('./pages/AgentInstallWizard'))
const AgentKeysPage = lazy(() => import('./pages/AgentKeys'))

// Billing
const BillingPage = lazy(() => import('./pages/Billing'))

export default function App() {
  return (
    <AuthProvider>
      <Suspense fallback={<div className="flex h-screen items-center justify-center text-sm text-muted-foreground">Loading…</div>}>
        <Routes>
          {/* Public routes */}
          <Route path="/login" element={<LoginPage />} />
          <Route path="/forgot-password" element={<ForgotPasswordPage />} />
          <Route path="/reset-password" element={<ResetPasswordPage />} />

          {/* Protected routes */}
          <Route path="/" element={<RequireAuth><Layout /></RequireAuth>}>
            <Route index element={<Navigate to="/dashboard" replace />} />

            {/* Command Center */}
            <Route path="dashboard" element={<DashboardPage />} />

            {/* Infrastructure */}
            <Route path="infrastructure" element={<InfrastructureDashboardPage />} />
            <Route path="servers" element={<ServersPage />} />
            <Route path="storage" element={<Navigate to="/storage-arrays" replace />} />
            <Route path="storage-arrays" element={<StorageArraysPage />} />
            <Route path="network" element={<NetworkPage />} />

            {/* Database */}
            <Route path="databases" element={<DatabaseDashboardPage />} />
            <Route path="oracle" element={<OracleMonitoringPage />} />
            <Route path="oracle/metrics" element={<OracleMetricEditorPage />} />

            {/* Cilium / eBPF */}
            <Route path="cilium" element={<CiliumMonitoringPage />} />

            {/* Middleware */}
            <Route path="middleware" element={<MiddlewareMonitoringPage />} />
            <Route path="middleware/nginx" element={<NginxMonitoringPage />} />
            <Route path="middleware/apache" element={<ApacheMonitoringPage />} />
            <Route path="middleware/tomcat" element={<TomcatMonitoringPage />} />
            <Route path="middleware/haproxy" element={<HAProxyMonitoringPage />} />
            <Route path="middleware/elasticsearch" element={<ElasticsearchMonitoringPage />} />
            <Route path="middleware/redis" element={<RedisMonitoringPage />} />
            <Route path="middleware/memcached" element={<MemcachedMonitoringPage />} />
            <Route path="middleware/rabbitmq" element={<RabbitMQMonitoringPage />} />
            <Route path="middleware/kafka" element={<KafkaMonitoringPage />} />
            <Route path="middleware/docker" element={<DockerMonitoringPage />} />
            <Route path="middleware/kubernetes" element={<KubernetesMonitoringPage />} />
            <Route path="middleware/phpfpm" element={<PhpFpmMonitoringPage />} />

            {/* Security */}
            <Route path="security/dashboard" element={<SecurityDashboardPage />} />
            <Route path="security/hunt" element={<ThreatHuntingPage />} />
            <Route path="security/firewall" element={<FirewallMonitoringPage />} />

            {/* Logs */}
            <Route path="log-explorer" element={<LogExplorerPage />} />
            <Route path="log-sources" element={<LogSourcesPage />} />
            <Route path="log-alerts" element={<LogAlertsPage />} />
            <Route path="log-analytics" element={<LogAnalyticsPage />} />

            {/* Alerts & Events */}
            <Route path="alerts" element={<AlertsPage />} />
            <Route path="alerts/rules" element={<AlertRulesPage />} />
            <Route path="notifications" element={<NotificationChannelsPage />} />

            {/* Admin */}
            <Route path="admin/users" element={<UserManagementPage />} />
            <Route path="admin/groups" element={<UserGroupManagementPage />} />
            <Route path="admin/roles" element={<RoleManagementPage />} />
            <Route path="admin/profile" element={<UserProfilePage />} />
            <Route path="admin/organizations" element={<OrganizationManagementPage />} />

            {/* AI & Analytics */}
            <Route path="ai-insights" element={<AIInsightsPage />} />
            <Route path="predictive-alerts" element={<PredictiveAlertsPage />} />
            <Route path="rca" element={<RCAPage />} />
            <Route path="runbooks" element={<RunbooksPage />} />
            <Route path="llm-observability" element={<LLMObservabilityPage />} />
            <Route path="profiling" element={<ProfilingPage />} />

            {/* OpenTelemetry */}
            <Route path="traces" element={<TracesPage />} />
            <Route path="otel-config" element={<OTelCollectorConfigPage />} />
            <Route path="otel-sdk-guide" element={<OTelSdkGuidePage />} />
            <Route path="otel-resources" element={<OTelResourcesPage />} />
            <Route path="otel-dashboard" element={<OTelDashboardPage />} />
            <Route path="otel-connectors" element={<OTelConnectorsPage />} />
            <Route path="otel-pipeline" element={<OTelPipelinePage />} />
            <Route path="otel-span-metrics" element={<OTelSpanMetricsPage />} />
            <Route path="otel-alert-rules" element={<OTelAlertRulesPage />} />
            <Route path="otel-security" element={<OTelSecurityPage />} />

            {/* Cloud Monitoring (P10) */}
            <Route path="cloud-monitoring" element={<CloudMonitoringPage />} />

            {/* Storage Deep-Dive (P11) */}
            <Route path="storage-deep-dive" element={<StorageDeepDivePage />} />

            {/* Fleet */}
            <Route path="fleet" element={<FleetPage />} />
            <Route path="fleet/install" element={<AgentInstallWizardPage />} />
            <Route path="fleet/keys" element={<AgentKeysPage />} />

            {/* Billing */}
            <Route path="billing" element={<BillingPage />} />

            {/* Fallback — redirect unknown paths to dashboard */}
            <Route path="*" element={<Navigate to="/dashboard" replace />} />
          </Route>
        </Routes>
      </Suspense>
    </AuthProvider>
  )
}
