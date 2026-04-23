# Spark Expo Go Example (No WASM)

This app is a minimal Expo Go example that connects to Spark using a no-WASM vendor bundle.
It uses `SparkNode` plus `installSparkRuntime(...)` from `@sunnyln/lni-spark`.

## Prerequisites

- Node `>=20.19.4` recommended by Expo SDK 54 / RN 0.81.
- Expo Go app installed on your device.

## Setup

From `bindings/typescript-spark/examples/spark-expo-go`:

```bash
# from repo root, build the local packages first
cd bindings/typescript
npm run build

cd ../typescript-spark
npm run build

# then run the Expo app
cd examples/spark-expo-go
npm install
npm run start
```

Then scan the QR code in Expo Go.

## Troubleshooting

### Hermes error: `Invalid call at line 280: import(specifier)`

This means Metro is still using an old cached Spark package build.

From `bindings/typescript-spark`:

```bash
npm run build
```

From `bindings/typescript-spark/examples/spark-expo-go`:

```bash
npx expo start --clear
```

If it still persists, stop all Expo/Metro processes and start again with `--clear`.

### `Unable to resolve module '@sunnyln/lni-spark'`

- Ensure the local package is installed in the example app:

```bash
cd bindings/typescript-spark/examples/spark-expo-go
npm install
```

- Confirm `bindings/typescript` and `bindings/typescript-spark` were built first (`npm run build`), since this example resolves from local `dist/*`.

### `Failed to load Spark SDK entry ... Unable to resolve .../dist/vendor/...`

This means the Spark vendor bundle files are missing from `bindings/typescript-spark/dist/vendor`.

Regenerate them:

```bash
cd bindings/typescript-spark
npm run build
```

Then restart Expo:

```bash
cd examples/spark-expo-go
npx expo start --clear
```

## What this app does

- Accepts Spark mnemonic, network, optional API key, and optional SSP settings.
- Persists form values in AsyncStorage.
- Uses `installSparkRuntime({ apiKey })` to set up Buffer/base64/fetch compatibility.
- Uses `new SparkNode(...)` + `getInfo()` + `listTransactions()`.
- Includes a `payInvoice` test section (paste a BOLT11 invoice and optionally amount for zero-amount invoices).
- Uses the Spark browser-safe vendor bundle packaged inside `@sunnyln/lni-spark` (no app-level shims/vendor needed).
- Includes Metro resolver handling for `./dist/*` imports from the linked package (`metro.config.js`).

## Security

This example stores sensitive inputs in AsyncStorage for developer convenience. Do not use this storage model directly in production apps.
