import { PhoenixdNode } from './index.js'
 
const config = {
    url: 'http://localhost:9740',
    password: ''
}
const node = new PhoenixdNode(config)
const offer = await node.getOffer()
console.log('Offer:', offer)

const configRes = await node.getConfig()
console.log('Config:', configRes.url)