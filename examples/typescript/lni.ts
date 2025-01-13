import { WasmLndNode as LndNode } from "../../lni/pkg/bundler/lni.js";

async function run() {
  const node = new LndNode("test_macaroon", "https://127.0.0.1:8080");

  // Fetch wallet balance
  // const res = node.pay_invoice("lno**");
  // console.log("Wallet Balance:", res);
  // const txn = node.get_wallet_transactions("wallet1");
  // txn.forEach((t) => {
  //   console.log("Transaction:", t.amount, t.date, t.memo);
  // })

  const inv = await node.check_payment_status("lni1234");
  console.log("Invoice:", inv);

  // node.on_payment_received("lni1234", (event: InvoiceEvent) => {
  //   console.log("Callback result:", event);
  // })

  

}

run();
