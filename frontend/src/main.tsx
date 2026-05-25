import React from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import App from './App'
import { ThemeProvider } from './contexts/ThemeContext'
import { AppModeProvider } from './contexts/AppModeContext'
import './index.css'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchInterval: 30_000,
      staleTime: 30_000,
      retry: 2,
    },
  },
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <ThemeProvider>
          <AppModeProvider>
            <App />
          </AppModeProvider>
        </ThemeProvider>
      </BrowserRouter>
    </QueryClientProvider>
  </React.StrictMode>,
)
