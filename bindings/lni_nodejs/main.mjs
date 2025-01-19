import { Fetcher } from './index.js'
 
const fetcher = new Fetcher()
const ip = await fetcher.getIpAddress()
console.log('Your IP address is', ip.origin)
