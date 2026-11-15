import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// Dev: proxy /api to a locally running agent (flakenet --api-listen 127.0.0.1:7777).
export default defineConfig({
  plugins: [react()],
  server: { proxy: { '/api': { target: 'http://127.0.0.1:7777', changeOrigin: false } } },
})
