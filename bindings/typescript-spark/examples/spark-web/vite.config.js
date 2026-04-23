import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    rollupOptions: {
      // The Spark SDK ships a React Native native entry that imports
      // "react-native". It is never used in the browser code path (we use
      // sdkEntry: 'bare'), but Rollup resolves it statically. Mark it
      // external so the production build succeeds.
      external: ['react-native'],
    },
  },
});
