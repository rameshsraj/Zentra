import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5010,
    proxy: {
      '/api': {
        target: 'http://localhost:5006',
        changeOrigin: true,
      },
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom', 'react-router-dom'],
          'vendor-charts': ['recharts'],
          'vendor-ui': ['lucide-react', 'class-variance-authority'],
          'vendor-query': ['@tanstack/react-query'],
          'vendor-utils': ['date-fns', 'clsx', 'tailwind-merge'],
        },
      },
    },
  },
})
