import { sayHello, Fetcher } from './index.js'
 
console.log('From native', sayHello())

const fetcher = new Fetcher()
fetcher.getIpAddress().then((res) => {
  console.log('From native', res.origin)
})