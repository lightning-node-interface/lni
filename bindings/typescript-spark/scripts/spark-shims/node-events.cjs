class EventEmitter {
  constructor() {
    this._events = new Map();
  }

  on(event, listener) {
    const list = this._events.get(event) ?? [];
    list.push(listener);
    this._events.set(event, list);
    return this;
  }

  addListener(event, listener) {
    return this.on(event, listener);
  }

  off(event, listener) {
    const list = this._events.get(event);
    if (!list) return this;
    this._events.set(
      event,
      list.filter((entry) => entry !== listener),
    );
    return this;
  }

  removeListener(event, listener) {
    return this.off(event, listener);
  }

  once(event, listener) {
    const wrapped = (...args) => {
      this.off(event, wrapped);
      listener(...args);
    };
    return this.on(event, wrapped);
  }

  emit(event, ...args) {
    const list = this._events.get(event);
    if (!list || list.length === 0) return false;
    for (const listener of [...list]) {
      listener(...args);
    }
    return true;
  }
}

module.exports = EventEmitter;
module.exports.EventEmitter = EventEmitter;
