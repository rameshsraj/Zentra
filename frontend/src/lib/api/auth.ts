import { fetchJSON } from './_client'
import type {
  LoginRequest, LoginResponse, UserProfile, UserSession, MfaSetupResponse,
  UserListResponse, UserDetail, CreateUserReq, UpdateUserReq, BulkUserActionReq,
  UserGroupItem, UserGroupDetail,
  RoleItem, RoleDetail, PermissionItem,
  AuditLogResponse, AuditStats,
  ApiTokenItem, CreateTokenResponse,
  SystemSettingItem,
} from './types'

// Authentication (V5 Prompt 1)
export const authLogin = (data: LoginRequest) =>
  fetchJSON<LoginResponse>('/auth/login', { method: 'POST', body: JSON.stringify(data) })
export const authLogout = () =>
  fetchJSON<{ message: string }>('/auth/logout', { method: 'POST' })
export const authRefresh = (refresh_token: string) =>
  fetchJSON<{ token: string; expires_at: string }>('/auth/refresh', { method: 'POST', body: JSON.stringify({ refresh_token }) })
export const authMe = () => fetchJSON<UserProfile>('/auth/me')
export const authMfaSetup = () =>
  fetchJSON<MfaSetupResponse>('/auth/mfa/setup', { method: 'POST' })
export const authMfaVerify = (code: string) =>
  fetchJSON<{ message: string }>('/auth/mfa/verify', { method: 'POST', body: JSON.stringify({ code }) })
export const authSessions = () => fetchJSON<UserSession[]>('/auth/sessions')
export const authRevokeSession = (id: string) =>
  fetchJSON<{ message: string }>(`/auth/sessions/${id}`, { method: 'DELETE' })

// User Management (V5 Prompt 2)
export const getUsers = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<UserListResponse>(`/users${qs}`)
}
export const createUser = (data: CreateUserReq) =>
  fetchJSON<any>('/users', { method: 'POST', body: JSON.stringify(data) })
export const getUser = (id: string) => fetchJSON<UserDetail>(`/users/${id}`)
export const updateUser = (id: string, data: UpdateUserReq) =>
  fetchJSON<any>(`/users/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteUser = (id: string) =>
  fetchJSON<any>(`/users/${id}`, { method: 'DELETE' })
export const lockUser = (id: string) =>
  fetchJSON<any>(`/users/${id}/lock`, { method: 'POST' })
export const unlockUser = (id: string) =>
  fetchJSON<any>(`/users/${id}/unlock`, { method: 'POST' })
export const resetUserPassword = (id: string, new_password: string) =>
  fetchJSON<any>(`/users/${id}/password`, { method: 'PUT', body: JSON.stringify({ new_password }) })
export const bulkUserAction = (data: BulkUserActionReq) =>
  fetchJSON<any>('/users/bulk', { method: 'POST', body: JSON.stringify(data) })

// User Groups (V5 Prompt 2)
export const getUserGroups = () => fetchJSON<UserGroupItem[]>('/user-groups')
export const createUserGroup = (data: { name: string; description?: string }) =>
  fetchJSON<any>('/user-groups', { method: 'POST', body: JSON.stringify(data) })
export const getUserGroup = (id: string) => fetchJSON<UserGroupDetail>(`/user-groups/${id}`)
export const updateUserGroup = (id: string, data: { name?: string; description?: string }) =>
  fetchJSON<any>(`/user-groups/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteUserGroup = (id: string) =>
  fetchJSON<any>(`/user-groups/${id}`, { method: 'DELETE' })
export const addGroupMembers = (groupId: string, user_ids: string[]) =>
  fetchJSON<any>(`/user-groups/${groupId}/members`, { method: 'POST', body: JSON.stringify({ user_ids }) })
export const removeGroupMember = (groupId: string, userId: string) =>
  fetchJSON<any>(`/user-groups/${groupId}/members/${userId}`, { method: 'DELETE' })

// Roles & Permissions (V5 Prompt 3)
export const getRoles = () => fetchJSON<RoleItem[]>('/roles')
export const createRole = (data: { name: string; description?: string; permission_ids: string[] }) =>
  fetchJSON<any>('/roles', { method: 'POST', body: JSON.stringify(data) })
export const getRole = (id: string) => fetchJSON<RoleDetail>(`/roles/${id}`)
export const updateRole = (id: string, data: { name?: string; description?: string; permission_ids?: string[] }) =>
  fetchJSON<any>(`/roles/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteRole = (id: string) => fetchJSON<any>(`/roles/${id}`, { method: 'DELETE' })
export const getPermissions = () => fetchJSON<Record<string, PermissionItem[]>>('/permissions')
export const assignUserRoles = (userId: string, role_ids: string[]) =>
  fetchJSON<any>(`/users/${userId}/roles`, { method: 'POST', body: JSON.stringify({ role_ids }) })

// Module Access (RBAC)
export const getUserModules = (userId: string) =>
  fetchJSON<{ user_id: string; modules: string[] }>(`/users/${userId}/modules`)
export const setUserModules = (userId: string, modules: string[]) =>
  fetchJSON<any>(`/users/${userId}/modules`, { method: 'PUT', body: JSON.stringify({ modules }) })

// Audit Log (V5 Prompt 3)
export const getAuditLog = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<AuditLogResponse>(`/audit-log${qs}`)
}
export const exportAuditLog = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<any>(`/audit-log/export${qs}`)
}
export const getAuditStats = () => fetchJSON<AuditStats>('/audit-log/stats')

// API Tokens (V5 Prompt 3)
export const getApiTokens = () => fetchJSON<ApiTokenItem[]>('/api-tokens')
export const createApiToken = (data: { name: string; scopes?: string[]; expires_in_days?: number }) =>
  fetchJSON<CreateTokenResponse>('/api-tokens', { method: 'POST', body: JSON.stringify(data) })
export const revokeApiToken = (id: string) => fetchJSON<any>(`/api-tokens/${id}`, { method: 'DELETE' })

// System Settings (V5 Prompt 3)
export const getSettings = () => fetchJSON<SystemSettingItem[]>('/settings')
export const updateSettings = (settings: { key: string; value: any }[]) =>
  fetchJSON<any>('/settings', { method: 'PUT', body: JSON.stringify({ settings }) })
