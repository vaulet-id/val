import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwind from '@tailwindcss/vite'
import path from 'node:path'

export default defineConfig({
  plugins: [react(), tailwind()],
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
  // The playground reads the specification and the examples out of the
  // repository rather than keeping copies. A copy is how a playground starts
  // teaching a language that no longer exists.
  server: { port: 5273, strictPort: true, fs: { allow: ['..'] } },
})
