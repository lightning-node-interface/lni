import { describe, expect, it } from 'vitest';
import { encodeBase64Bytes } from '../internal/encoding.js';
import { getBlinkTokenPermissions, getStrikeOauthPermissions, parseClnRunePermissions } from '../internal/permissions.js';
import { BlinkNode } from '../nodes/blink.js';
import { LndNode } from '../nodes/lnd.js';
import { PhoenixdNode } from '../nodes/phoenixd.js';
import { SpeedNode } from '../nodes/speed.js';
import { StrikeNode } from '../nodes/strike.js';

describe('permissions helpers', () => {
  it('expands CLN method prefix rune restrictions to known methods', () => {
    const raw = new TextEncoder().encode('unique-id&method^list|method=getinfo');
    const rune = encodeBase64Bytes(raw).replace(/\+/g, '-').replace(/\//g, '_');

    expect(parseClnRunePermissions(rune)).toEqual([
      'getinfo',
      'listfunds',
      'listinvoices',
      'listoffers',
    ]);
  });

  it('checks LND macaroon permissions against the node permission map', async () => {
    const node = new LndNode(
      {
        url: 'https://lnd.example.test',
        macaroon: '00',
      },
      {
        fetch: async (input, init) => {
          const url = String(input);

          if (url.endsWith('/v1/macaroon/permissions')) {
            return Response.json({
              method_permissions: {
                '/lnrpc.Lightning/GetInfo': {
                  permissions: [{ entity: 'info', action: 'read' }],
                },
                '/lnrpc.Lightning/AddInvoice': {
                  permissions: [{ entity: 'invoices', action: 'write' }],
                },
              },
            });
          }

          if (url.endsWith('/v1/macaroon/checkpermissions')) {
            const body = JSON.parse(String(init?.body));
            return Response.json({
              valid: body.permissions?.[0]?.entity === 'info',
            });
          }

          return new Response('not found', { status: 404 });
        },
      },
    );

    await expect(node.getPermissions()).resolves.toEqual(['/lnrpc.Lightning/GetInfo']);
  });

  it('maps Strike OAuth JWT scopes to LNI permissions', () => {
    const accessToken = [
      encodeJwtPart({ alg: 'none' }),
      encodeJwtPart({
        scope: [
          'openid',
          'partner.balances.read',
          'partner.receive-request.create',
          'partner.receive-request.read',
          'partner.payment-quote.lightning.create',
          'partner.payment-quote.execute',
        ].join(' '),
      }),
      'signature',
    ].join('.');

    expect(getStrikeOauthPermissions(accessToken)).toEqual([
      'createInvoice',
      'decode',
      'getInfo',
      'lookupInvoice',
      'onInvoiceEvents',
      'payInvoice',
    ]);
  });

  it('rejects opaque Strike API keys for permission introspection', async () => {
    const node = new StrikeNode({ apiKey: 'sk_test_opaque' });

    await expect(node.getPermissions()).rejects.toMatchObject({
      code: 'InvalidInput',
    });
  });

  it('maps Blink JWT scopes to LNI permissions', () => {
    const token = [
      encodeJwtPart({ alg: 'none' }),
      encodeJwtPart({ scope: 'Read Receive Write' }),
      'signature',
    ].join('.');

    expect(getBlinkTokenPermissions(token)).toEqual([
      'createInvoice',
      'decode',
      'getInfo',
      'listTransactions',
      'lookupInvoice',
      'onInvoiceEvents',
      'payInvoice',
    ]);
  });

  it('rejects opaque Blink API keys for permission introspection', async () => {
    const node = new BlinkNode({ apiKey: 'blink_opaque' });

    await expect(node.getPermissions()).rejects.toMatchObject({
      code: 'InvalidInput',
    });
  });

  it('rejects Speed API keys for permission introspection', async () => {
    const node = new SpeedNode({ apiKey: 'sk_test_opaque' });

    await expect(node.getPermissions()).rejects.toMatchObject({
      code: 'InvalidInput',
    });
  });

  it('rejects Phoenixd passwords for permission introspection', async () => {
    const node = new PhoenixdNode({ url: 'https://phoenixd.example.test', password: 'secret' });

    await expect(node.getPermissions()).rejects.toMatchObject({
      code: 'InvalidInput',
    });
  });
});

function encodeJwtPart(value: unknown): string {
  return encodeBase64Bytes(new TextEncoder().encode(JSON.stringify(value)))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/g, '');
}
