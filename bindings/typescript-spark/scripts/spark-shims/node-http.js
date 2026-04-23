function notSupported() {
  throw new Error('node:http is not supported in this browser build.');
}

const http = {
  request: notSupported,
};

export default http;
