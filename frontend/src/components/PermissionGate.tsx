import { ReactNode } from 'react'
import { useAuth } from '../contexts/AuthContext'

interface PermissionGateProps {
  resource: string
  action: string
  children: ReactNode
  fallback?: ReactNode
}

export default function PermissionGate({ resource, action, children, fallback = null }: PermissionGateProps) {
  const { hasPermission, isAdmin } = useAuth()

  if (isAdmin || hasPermission(resource, action)) {
    return <>{children}</>
  }

  return <>{fallback}</>
}

interface RoleGateProps {
  role: string
  children: ReactNode
  fallback?: ReactNode
}

export function RoleGate({ role, children, fallback = null }: RoleGateProps) {
  const { hasRole, isAdmin } = useAuth()

  if (isAdmin || hasRole(role)) {
    return <>{children}</>
  }

  return <>{fallback}</>
}
