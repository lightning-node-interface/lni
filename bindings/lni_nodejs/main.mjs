import { PhoenixdNode, InvoiceType } from './index.js'
import dotenv from 'dotenv';
dotenv.config();

 
const config = {
    url: process.env.PHOENIXD_URL,
    password: process.env.PHOENIXD_PASSWORD,
    test_hash: process.env.PHOENIXD_TEST_PAYMENT_HASH,
}
const node = new PhoenixdNode(config)
const info = await node.getInfo()
console.log('Node info:', info)

const configRes = await node.getConfig()
console.log('Config:', configRes.url)

const invoice = await node.makeInvoice({ amount: 1000, description: 'test invoice', invoiceType: InvoiceType.Bolt11 })
console.log('Invoice:', invoice)

const lookupInvoice = await node.lookupInvoice(config.test_hash)
console.log('lookupInvoice:', lookupInvoice)

const txns = await node.listTransactions({
    from: 0,
    until: 0,
    limit: 10,
    offset: 0,
    unpaid: false,
    invoiceType: 'all',
})
console.log('Transactions:', txns)