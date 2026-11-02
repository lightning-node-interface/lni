'use strict'

function abortError() {
  const error = new Error('LexeConnect cancelled')
  error.name = 'AbortError'
  return error
}

function result(response) {
  const context = { account: response.account, metadata: response.metadata }
  if (response.clientCredentials !== undefined && response.clientCredentials !== null) {
    return {
      ...context,
      status: 'approved',
      clientCredentials: response.clientCredentials,
      scopes: response.scopes,
      permissions: response.permissions,
      expiresAtMs: response.expiresAtMs,
    }
  }
  const rejected = response.error === 'user_rejected'
  return {
    ...context,
    status: rejected ? 'rejected' : 'error',
    error: rejected ? 'user_rejected' : 'other',
    message: response.errorMessage,
  }
}

function delay(ms, signal) {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(abortError())
    const abort = () => {
      clearTimeout(timer)
      reject(abortError())
    }
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', abort)
      resolve()
    }, ms)
    signal.addEventListener('abort', abort, { once: true })
  })
}

function createLexeConnectClass(NativeSession) {
  return class LexeConnect {
    constructor(config, native) {
      if (config.expiresAtMs !== undefined &&
          (!Number.isSafeInteger(config.expiresAtMs) || config.expiresAtMs < 0)) {
        throw new Error('expiresAtMs must be a non-negative safe integer')
      }
      if (config.requestTtlSecs !== undefined &&
          (!Number.isInteger(config.requestTtlSecs) || config.requestTtlSecs < 1 || config.requestTtlSecs > 300)) {
        throw new Error('requestTtlSecs must be between 1 and 300')
      }
      this.native = native || new NativeSession(config)
      this.disposed = false
      this.waiter = undefined
    }

    assertLive() {
      if (this.disposed) throw new Error('LexeConnect disposed')
    }

    connectionString() {
      this.assertLive()
      return this.native.connectionString()
    }

    acceptRedirect(url) {
      this.assertLive()
      return result(this.native.acceptRedirect(url))
    }

    acceptBody(body) {
      this.assertLive()
      return result(this.native.acceptBody(Buffer.from(body)))
    }

    async pollMailbox(signal) {
      this.assertLive()
      if (signal?.aborted) throw abortError()
      const response = await new Promise((resolve, reject) => {
        const abort = () => {
          signal.removeEventListener('abort', abort)
          reject(abortError())
        }
        signal?.addEventListener('abort', abort, { once: true })
        Promise.resolve().then(() => this.native.pollMailbox()).then(
          (value) => {
            signal?.removeEventListener('abort', abort)
            resolve(value)
          },
          (error) => {
            signal?.removeEventListener('abort', abort)
            reject(error)
          }
        )
      })
      return response == null ? undefined : result(response)
    }

    async waitForResponse(options = {}) {
      this.assertLive()
      if (this.waiter) throw new Error('LexeConnect is already waiting')
      const interval = options.pollIntervalMs ?? 1000
      if (!Number.isInteger(interval) || interval < 250 || interval > 5000) {
        throw new Error('pollIntervalMs must be between 250 and 5000')
      }
      const controller = new AbortController()
      this.waiter = controller
      const abort = () => this.cancel()
      options.signal?.addEventListener('abort', abort, { once: true })
      try {
        if (options.signal?.aborted) this.cancel()
        for (;;) {
          if (controller.signal.aborted) throw abortError()
          const response = await this.pollMailbox(controller.signal)
          if (response !== undefined) return response
          await delay(interval, controller.signal)
        }
      } finally {
        options.signal?.removeEventListener('abort', abort)
        this.waiter = undefined
      }
    }

    cancel() {
      if (this.disposed) return
      this.waiter?.abort()
      this.native.cancel()
    }

    dispose() {
      if (this.disposed) return
      this.cancel()
      this.native = undefined
      this.disposed = true
    }
  }
}

module.exports = { createLexeConnectClass }
