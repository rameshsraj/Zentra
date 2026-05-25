import { Navigate, useLocation } from 'react-router-dom'
import { useAuth } from '../contexts/AuthContext'
import { Loader2 } from 'lucide-react'

interface RequireAuthProps {
  children: React.ReactNode
  requiredRole?: string
  requiredPermission?: { resource: string; action: string }
}

export default function RequireAuth({ children, requiredRole, requiredPermission }: RequireAuthProps) {
  const { isAuthenticated, isLoading, hasRole, hasPermission } = useAuth()
  const location = useLocation()

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-950">
        <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
      </div>
    )
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" state={{ from: location }} replace />
  }

  if (requiredRole && !hasRole(requiredRole) && !hasRole('Admin') && !hasRole('Super Admin')) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-950">
        <div className="text-center">
          <h2 className="text-xl font-bold text-white mb-2">Access Denied</h2>
          <p className="text-gray-400">You don't have the required role: {requiredRole}</p>
        </div>
      </div>
    )
  }

  if (requiredPermission && !hasPermission(requiredPermission.resource, requiredPermission.action)) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-950">
        <div className="text-center">
          <h2 className="text-xl font-bold text-white mb-2">Access Denied</h2>
          <p className="text-gray-400">
            Missing permission: {requiredPermission.resource}.{requiredPermission.action}
          </p>
        </div>
      </div>
    )
  }

  return <>{children}</>
}
