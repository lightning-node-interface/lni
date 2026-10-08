import { afterEach, describe, expect, it, vi } from 'vitest';
import { LexeConnect } from '../LexeConnect';
import type {
  LexeConnectResponse,
  LexeConnectSessionLike,
} from '../generated/react_native_lexe';

type NativeSession = LexeConnectSessionLike & { uniffiDestroy(): void };

const approved: LexeConnectResponse = {
  clientCredentials: 'test-only-credential',
  scopes: ['read_info'],
  permissions: [],
  expiresAtMs: 1821484800000n,
  account: '@example',
  metadata: 'example-state',
};

function nativeSession(overrides: Partial<NativeSession> = {}): NativeSession {
  return {
    connectionString: vi.fn(() => 'https://lexe.app/connect?test-only'),
    acceptRedirect: vi.fn(() => approved),
    acceptBody: vi.fn(() => approved),
    pollMailbox: vi.fn(async () => approved),
    cancel: vi.fn(),
    uniffiDestroy: vi.fn(),
    ...overrides,
  };
}

afterEach(() => vi.useRealTimers());

describe('LexeConnect adapter', () => {
  it('maps verified native results and passes callback URLs unchanged', () => {
    const native = nativeSession();
    const connect = new LexeConnect({}, native);
    const response = connect.acceptRedirect(
      'exampleapp://connect?response=test'
    );
    expect(native.acceptRedirect).toHaveBeenCalledWith(
      'exampleapp://connect?response=test'
    );
    expect(response).toEqual({
      status: 'approved',
      clientCredentials: 'test-only-credential',
      scopes: ['read_info'],
      permissions: [],
      expiresAtMs: 1821484800000,
      account: '@example',
      metadata: 'example-state',
    });
  });

  it('distinguishes rejection from a wallet error', () => {
    for (const [error, status] of [
      ['user_rejected', 'rejected'],
      ['other', 'error'],
    ]) {
      const native = nativeSession({
        acceptRedirect: vi.fn(() => ({
          error,
          errorMessage: 'test-only-message',
          scopes: [],
          permissions: [],
        })),
      });
      const connect = new LexeConnect({}, native);
      expect(connect.acceptRedirect('exampleapp://connect')).toMatchObject({
        status,
        error,
        message: 'test-only-message',
      });
    }
  });

  it('preserves raw encrypted bytes', () => {
    const native = nativeSession();
    const connect = new LexeConnect({}, native);
    const bytes = new Uint8Array([99, 1, 2, 3, 99]).subarray(1, 4);
    connect.acceptBody(bytes);
    expect(native.acceptBody).toHaveBeenCalledWith(
      new Uint8Array([1, 2, 3]).buffer
    );
  });

  it('polls until a response arrives', async () => {
    vi.useFakeTimers();
    const pollMailbox = vi
      .fn()
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(approved);
    const connect = new LexeConnect({}, nativeSession({ pollMailbox }));
    const response = connect.waitForResponse();
    await vi.advanceTimersByTimeAsync(1000);
    expect((await response).status).toBe('approved');
    expect(pollMailbox).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('cancels while waiting and releases its native handle exactly once', async () => {
    vi.useFakeTimers();
    const native = nativeSession({ pollMailbox: vi.fn(async () => undefined) });
    const connect = new LexeConnect({}, native);
    const response = connect.waitForResponse();
    const rejected = expect(response).rejects.toMatchObject({
      name: 'AbortError',
    });
    await vi.advanceTimersByTimeAsync(0);
    connect.dispose();
    connect.dispose();
    await rejected;
    expect(native.cancel).toHaveBeenCalledTimes(1);
    expect(native.uniffiDestroy).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
    expect(() => connect.connectionString()).toThrow('disposed');
  });

  it('honors an already aborted signal without polling', async () => {
    const native = nativeSession();
    const connect = new LexeConnect({}, native);
    const controller = new AbortController();
    controller.abort();
    await expect(
      connect.waitForResponse({ signal: controller.signal })
    ).rejects.toMatchObject({ name: 'AbortError' });
    expect(native.pollMailbox).not.toHaveBeenCalled();
    expect(native.cancel).toHaveBeenCalledOnce();
  });

  it('allows retry after transport failure but rejects concurrent waiters', async () => {
    vi.useFakeTimers();
    const pollMailbox = vi
      .fn()
      .mockRejectedValueOnce(new Error('transport failed'))
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(approved);
    const native = nativeSession({ pollMailbox });
    const connect = new LexeConnect({}, native);
    await expect(connect.waitForResponse()).rejects.toThrow('transport failed');
    const retry = connect.waitForResponse();
    await expect(connect.waitForResponse()).rejects.toThrow('already waiting');
    await vi.advanceTimersByTimeAsync(1000);
    expect((await retry).status).toBe('approved');
    expect(native.cancel).not.toHaveBeenCalled();
  });

  it('rejects unsafe numbers before calling native code', () => {
    const native = nativeSession();
    for (const expiresAtMs of [
      -1,
      0.5,
      NaN,
      Infinity,
      Number.MAX_SAFE_INTEGER + 1,
    ]) {
      expect(() => new LexeConnect({ expiresAtMs }, native)).toThrow(
        'safe integer'
      );
    }
    for (const requestTtlSecs of [0, 301, 1.5, NaN]) {
      expect(() => new LexeConnect({ requestTtlSecs }, native)).toThrow(
        '1 and 300'
      );
    }
    const connect = new LexeConnect(
      {},
      nativeSession({
        acceptRedirect: vi.fn(() => ({
          ...approved,
          expiresAtMs: 2n ** 64n - 1n,
        })),
      })
    );
    expect(() => connect.acceptRedirect('exampleapp://connect')).toThrow(
      'safe integer'
    );
  });
});
