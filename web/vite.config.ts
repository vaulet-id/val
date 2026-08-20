import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwind from '@tailwindcss/vite'
import path from 'node:path'

export default defineConfig({
  // The playground is served under a path — `vaulet.io/playground` — because it
  // is part of the site rather than a place of its own. It is set for the dev
  // server too, so what is being looked at locally is mounted where the
  // deployed one is: a base that differs between the two is a class of bug that
  // only appears in production.
  base: '/playground/',
  plugins: [react(), tailwind()],
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
  // The playground reads the specification and the examples out of the
  // repository rather than keeping copies. A copy is how a playground starts
  // teaching a language that no longer exists.
  server: { port: 5273, strictPort: true, fs: { allow: ['..'] } },
})
