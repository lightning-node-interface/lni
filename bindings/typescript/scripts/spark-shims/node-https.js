function notSupported() {
  throw new Error('node:https is not supported in this browser build.');
}

const https = {
  request: notSupported,
};

export default https;
