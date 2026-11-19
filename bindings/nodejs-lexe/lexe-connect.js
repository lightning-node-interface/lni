'use strict'

// Keep this file within Node 10 syntax and APIs: the root entry loads it for every backend.

// Longest wait between retries of a failed mailbox request.
const MAX_RETRY_DELAY_MS = 10000

// Errors thrown here carry a code, like native ones (see lexe-errors.js).
function codedError(code, message) {
  const error = new Error(message)
  error.code = code
  return error
}

function abortError() {
  const error = new Error('LexeConnect cancelled')
  error.name = 'AbortError'
  return error
}

function result(response) {
  const context = { account: response.account, metadata: response.metadata }
  if (response.clientCredentials !== undefined && response.clientCredentials !== null) {
    return Object.assign(context, {
      status: 'approved',
      clientCredentials: response.clientCredentials,
      scopes: response.scopes,
      permissions: response.permissions,
      expiresAtMs: response.expiresAtMs,
    })
  }
  const rejected = response.error === 'user_rejected'
  return Object.assign(context, {
    status: rejected ? 'rejected' : 'error',
    error: rejected ? 'user_rejected' : 'other',
    message: response.errorMessage,
  })
}

// Minimal AbortSignal-compatible source; global AbortController needs Node 15.
function createAbortSource() {
  const listeners = []
  const signal = {
    aborted: false,
    addEventListener(type, listener) {
      if (type === 'abort') listeners.push(listener)
    },
    removeEventListener(type, listener) {
      const index = listeners.indexOf(listener)
      if (index !== -1) listeners.splice(index, 1)
    },
  }
  return {
    signal,
    abort() {
      if (signal.aborted) return
      signal.aborted = true
      listeners.splice(0).forEach((listener) => listener())
    },
  }
}

// Accepts any AbortSignal-like object. Returns a function that stops listening.
function onAbort(signal, listener) {
  if (!signal) return () => {}
  signal.addEventListener('abort', listener)
  return () => signal.removeEventListener('abort', listener)
}

function delay(ms, signal) {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(abortError())
    const stopListening = onAbort(signal, () => {
      clearTimeout(timer)
      stopListening()
      reject(abortError())
    })
    const timer = setTimeout(() => {
      stopListening()
      resolve()
    }, ms)
  })
}

function createLexeConnectClass(NativeSession) {
  return class LexeConnect {
    constructor(config, native) {
      if (config.expiresAtMs !== undefined &&
          (!Number.isSafeInteger(config.expiresAtMs) || config.expiresAtMs < 0)) {
        throw codedError('LEXE_INVALID_INPUT', 'expiresAtMs must be a non-negative safe integer')
      }
      if (config.requestTtlSecs !== undefined &&
          (!Number.isInteger(config.requestTtlSecs) || config.requestTtlSecs < 1 || config.requestTtlSecs > 300)) {
        throw codedError('LEXE_INVALID_INPUT', 'requestTtlSecs must be between 1 and 300')
      }
      this.native = native || new NativeSession(config)
      this.disposed = false
      this.waiter = undefined
      // At most one native poll runs; concurrent callers share it.
      this.poll = undefined
      // A verified response that arrived after every caller had aborted.
      this.retained = undefined
    }

    assertLive() {
      if (this.disposed) throw codedError('LEXE_CONNECT_DISPOSED', 'LexeConnect disposed')
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

    /**
     * Aborting `signal` stops only this caller from waiting. The native request
     * continues; a verified response it receives is returned by the next call.
     */
    async pollMailbox(signal) {
      this.assertLive()
      if (signal && signal.aborted) throw abortError()
      if (this.retained !== undefined) {
        const response = this.retained
        this.retained = undefined
        return response
      }
      const poll = this.poll || this.startPoll()
      return new Promise((resolve, reject) => {
        const waiter = { resolve, reject, stopListening: undefined }
        poll.waiters.add(waiter)
        waiter.stopListening = onAbort(signal, () => {
          waiter.stopListening()
          if (poll.waiters.delete(waiter)) reject(abortError())
        })
      })
    }

    startPoll() {
      const poll = { waiters: new Set() }
      this.poll = poll
      const finish = (deliver) => {
        if (this.poll === poll) this.poll = undefined
        const waiters = Array.from(poll.waiters)
        poll.waiters.clear()
        waiters.forEach((waiter) => {
          waiter.stopListening()
          deliver(waiter)
        })
        return waiters.length
      }
      const native = this.native
      Promise.resolve()
        .then(() => native.pollMailbox())
        .then((value) => {
          const response = value == null ? undefined : result(value)
          const delivered = finish((waiter) => waiter.resolve(response))
          // Native has consumed the one-use session, so the next caller gets it.
          if (response !== undefined && delivered === 0 && !this.disposed) this.retained = response
        }, (error) => {
          // An unobserved failure leaves the native session live or terminal;
          // the next poll retries or reports that state.
          finish((waiter) => waiter.reject(error))
        })
      return poll
    }

    async waitForResponse(options = {}) {
      this.assertLive()
      if (this.waiter) throw codedError('LEXE_INVALID_INPUT', 'LexeConnect is already waiting')
      const interval = options.pollIntervalMs === undefined || options.pollIntervalMs === null
        ? 1000
        : options.pollIntervalMs
      if (!Number.isInteger(interval) || interval < 250 || interval > 5000) {
        throw codedError('LEXE_INVALID_INPUT', 'pollIntervalMs must be between 250 and 5000')
      }
      const source = createAbortSource()
      this.waiter = source
      const stopListening = onAbort(options.signal, () => this.cancel())
      try {
        if (options.signal && options.signal.aborted) this.cancel()
        let failures = 0
        for (;;) {
          if (source.signal.aborted) throw abortError()
          let response
          try {
            response = await this.pollMailbox(source.signal)
          } catch (error) {
            // A failed mailbox request leaves the attempt live. Back off and
            // retry; the native deadline ends it with LEXE_CONNECT_EXPIRED.
            if (!error || error.code !== 'LEXE_CONNECT_UNAVAILABLE') throw error
            failures += 1
            await delay(Math.min(interval * Math.pow(2, failures), MAX_RETRY_DELAY_MS), source.signal)
            continue
          }
          failures = 0
          if (response !== undefined) return response
          await delay(interval, source.signal)
        }
      } finally {
        stopListening()
        this.waiter = undefined
      }
    }

    /**
     * Ends the attempt. A response the native session verified before
     * cancellation took effect is still returned by the next pollMailbox().
     */
    cancel() {
      if (this.disposed) return
      if (this.waiter) this.waiter.abort()
      this.native.cancel()
    }

    /** Cancels and releases the session, including any retained response. */
    dispose() {
      if (this.disposed) return
      this.cancel()
      this.native = undefined
      this.retained = undefined
      this.disposed = true
    }
  }
}

module.exports = { createLexeConnectClass }
