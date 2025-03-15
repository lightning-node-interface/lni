import { PhoenixdNode, ClnNode } from "./index.js";
import dotenv from "dotenv";
dotenv.config();

async function phoenixd() {
  const config = {
    url: process.env.PHOENIXD_URL,
    password: process.env.PHOENIXD_PASSWORD,
    test_hash: process.env.PHOENIXD_TEST_PAYMENT_HASH,
  };
  const node = new PhoenixdNode(config);
  const info = await node.getInfo();
  console.log("Node info:", info);

  const configRes = await node.getConfig();
  console.log("Config:", configRes.url);

  const invoice = await node.makeInvoice({
    amountMsats: 1000,
    description: "test invoice",
    invoiceType: "Bolt11",
  });
  console.log("Invoice:", invoice);

  const lookupInvoice = await node.lookupInvoice(
    process.env.PHOENIXD_TEST_PAYMENT_HASH
  );
  console.log("lookupInvoice:", lookupInvoice);

  const payOffer = await node.payOffer(
    process.env.TEST_RECEIVER_OFFER,
    3000,
    "payment from lni nodejs"
  );
  console.log("payOffer:", payOffer);

  const txns = await node.listTransactions({
    from: 0,
    until: 0,
  });
  console.log("Transactions:", txns);
}

async function cln() {
  const config = {
    url: process.env.CLN_URL,
    rune: process.env.CLN_RUNE,
  };
  const node = new ClnNode(config);
  const info = await node.getInfo();
  console.log("Node info:", info);

  const configRes = await node.getConfig();
  console.log("Config:", configRes.url);

  const invoice = await node.makeInvoice({
    amountMsats: 1000,
    description: "test invoice",
    invoiceType: InvoiceType.Bolt11,
  });
  console.log("Invoice:", invoice);

  const lookupInvoice = await node.lookupInvoice(
    process.env.CLN_TEST_PAYMENT_HASH
  );
  console.log("lookupInvoice:", lookupInvoice);

  const payOffer = await node.payOffer(
    process.env.TEST_RECEIVER_OFFER,
    3000,
    "payment from lni nodejs"
  );
  console.log("payOffer:", payOffer);

  const txns = await node.listTransactions({
    from: 0,
    limit: 10,
  });
  console.log("Transactions:", txns);
}

async function main() {
  phoenixd();
  cln();
}

main();
