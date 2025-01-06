import { LndNode } from "../../pkg/bundler/lni.js";

async function run() {
  const node = new LndNode("test_macaroon", "https://127.0.0.1:8080");

  // Fetch wallet balance
  const res = node.pay_invoice("lno**");
  console.log("Wallet Balance:", res);
  const txn = node.get_wallet_transactions("wallet1");
  txn.forEach((t) => {
    console.log("Transaction:", t.amount, t.date, t.memo);
  })
}

run();
