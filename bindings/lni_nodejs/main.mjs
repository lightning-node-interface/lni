import { PhoenixdNode, InvoiceType, Db } from "./index.js";
import dotenv from "dotenv";
dotenv.config();

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
  invoiceType: InvoiceType.Bolt11,
});
console.log("Invoice:", invoice);

const lookupInvoice = await node.lookupInvoice(process.env.PHOENIXD_TEST_PAYMENT_HASH);
console.log("lookupInvoice:", lookupInvoice);

const payOffer = await node.payOffer(process.env.TEST_OFFER, 3000, 'payment from lni nodejs');
console.log("payOffer:", payOffer);

const txns = await node.listTransactions({
  from: 0,
  until: 0,
  limit: 10,
  offset: 0,
  unpaid: false,
  invoiceType: "all",
});
console.log("Transactions:", txns);

// const db = new Db("test.json");
// db.writePayment({
//   paymentId: "1",
//   circId: "1",
//   round: 1,
//   relayFingerprint: "1",
//   updatedAt: 1,
//   amountMsats: 1,
//   amount_msats: 1000,
// });
