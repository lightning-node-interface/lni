function notSupported() {
  throw new Error('node:https is not supported in this browser build.');
}

const https = {
  request: notSupported,
  get: notSupported,
};

export default https;
