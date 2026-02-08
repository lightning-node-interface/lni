# Spark Web Demo (No WASM)

This demo is a Vite-served web page that uses LNI's public API (`createNode({ kind: 'spark' })`) directly from `@sunnyln/lni`.

It does **not** import WASM modules.

## Run

```bash
cd bindings/typescript
npm run build

cd examples/spark-web
npm install
npm run dev
```

Open:

- http://localhost:5173

`npm run build` generates and validates both vendor bundles in `dist/vendor`.
