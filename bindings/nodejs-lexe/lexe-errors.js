'use strict'

// Keep this file within Node 10 syntax and APIs: the root entry loads it for every backend.

// Native Lexe errors start with a stable code ("LEXE_<NAME>: message"). Callers
// branch on `error.code`, never on the wording. LEXE.md lists the codes.
const CODED = /^(LEXE_[A-Z_]+): ([\s\S]*)$/

function withCode(error) {
  if (error instanceof Error && typeof error.message === 'string') {
    const match = CODED.exec(error.message)
    if (match) {
      error.code = match[1]
      error.message = match[2]
    }
  }
  return error
}

function coded(fn, self) {
  return function codedMethod() {
    let result
    try {
      result = fn.apply(self === undefined ? this : self, arguments)
    } catch (error) {
      throw withCode(error)
    }
    if (result && typeof result.then === 'function') {
      return result.then(undefined, (error) => {
        throw withCode(error)
      })
    }
    return result
  }
}

function wrapFunctions(target) {
  Object.getOwnPropertyNames(target).forEach((name) => {
    if (name === 'constructor' || name === 'prototype') return
    const descriptor = Object.getOwnPropertyDescriptor(target, name)
    if (!descriptor || typeof descriptor.value !== 'function') return
    if (descriptor.writable) {
      target[name] = coded(descriptor.value)
    } else if (descriptor.configurable) {
      Object.defineProperty(target, name, Object.assign({}, descriptor, { value: coded(descriptor.value) }))
    }
  })
}

/**
 * Adds `error.code` to errors from a native class's methods, in place, and
 * returns a constructor that does the same for `new` and static methods. The
 * returned constructor shares the native prototype, so `instanceof` still works.
 *
 * Native statics are read-only and non-configurable, so they cannot be replaced
 * in place, and a proxy of the class itself must return them unchanged. The
 * proxy therefore wraps a plain function and forwards to the native class.
 */
function installErrorCodes(NativeClass) {
  wrapFunctions(NativeClass.prototype)
  const statics = new Map()
  const wrapped = new Proxy(function () {}, {
    construct(_target, args, newTarget) {
      try {
        return Reflect.construct(NativeClass, args, newTarget === wrapped ? NativeClass : newTarget)
      } catch (error) {
        throw withCode(error)
      }
    },
    apply(_target, thisArg, args) {
      try {
        return Reflect.apply(NativeClass, thisArg, args)
      } catch (error) {
        throw withCode(error)
      }
    },
    get(_target, property) {
      const value = Reflect.get(NativeClass, property)
      if (property === 'prototype' || typeof property === 'symbol' || typeof value !== 'function') return value
      if (!statics.has(property)) statics.set(property, coded(value, NativeClass))
      return statics.get(property)
    },
  })
  return wrapped
}

module.exports = { installErrorCodes, withCode }
