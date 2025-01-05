import { greet } from '../../pkg/bundler/lni.js';

async function run() {
    console.log(greet('World'));
}

run();