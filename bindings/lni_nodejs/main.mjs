import { PhoenixdNode } from './index.js'
import dotenv from 'dotenv';
dotenv.config();

 
const config = {
    url: process.env.PHOENIXD_URL,
    password: process.env.PHOENIXD_PASSWORD,
}
const node = new PhoenixdNode(config)
const info = await node.getInfo()
console.log('Node info:', info)

const configRes = await node.getConfig()
console.log('Config:', configRes.url)