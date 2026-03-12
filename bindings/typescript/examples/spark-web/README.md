# LNI Web Demo (No WASM)

This demo is a Vite-served web page that uses LNI's public API directly from `@sunnyln/lni`.

It supports two browser-selectable backends:

- `spark`
- `arkade-boltz`

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

## Manual pay-invoice test

1. Choose `Spark` or `Arkade Boltz` in the demo UI.
2. Enter the backend credentials and connect the wallet.
3. For `Arkade Boltz`, set `Ark Server URL` in Advanced settings if you are not using the default Mutinynet endpoint.
4. Paste a Bolt11 invoice into **Pay invoice test**.
5. For amountless invoices, set **Amount (msats)**.
6. Click **Pay Invoice** and check **Pay result**.

## Manual receive test

1. Choose `Spark` or `Arkade Boltz` in the demo UI.
2. Connect the wallet.
3. Enter an amount and click **Create Invoice**.
4. Copy the returned Bolt11 invoice and pay it from another wallet.
5. Watch the payment status and transaction list update in the UI.
