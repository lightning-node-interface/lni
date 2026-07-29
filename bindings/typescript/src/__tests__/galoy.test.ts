import { describe, expect, it, vi } from 'vitest';
import { decode } from '../decode.js';
import { createNode } from '../factory.js';
import { createGaloyNode } from '../nodes/galoy.js';
import { NwcError } from '../errors.js';
import type { FetchLike, GaloyConfig } from '../types.js';

const ONE_SAT_BOLT11S = [
  [
    'mainnet',
    'lnbc10n1pj48ugqdphf38yjgz8v9kx77fqxys8xct5ypex2emjv4ehx6t0dcsxv6tcw36hyegpp5qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurssp5pqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyq9qrsgqcqpj85c7x06rkw8s97xtjaarx4y4sgumglauw96fkcdr3yatkshg23gj57pj350za5ppku4d4hl8p6xj9ty7t84z2594q9hl7vf4em9en8cp3rvsy3',
  ],
  [
    'testnet',
    'lntb10n1pj48ugqdphf38yjgz8v9kx77fqxys8xct5ypex2emjv4ehx6t0dcsxv6tcw36hyegpp5qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurssp5pqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyq9qrsgqcqpjglkgmmeh8nlna32uhyqvpmrxk52er02glraz7jywwxg0tz0ahuxrtzanfvxjrugv2zuv8dxvakvk5p3fuxeexym8ff96m25s7ks750sqknxxzt',
  ],
  [
    'regtest',
    'lnbcrt10n1pj48ugqdphf38yjgz8v9kx77fqxys8xct5ypex2emjv4ehx6t0dcsxv6tcw36hyegpp5qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurswpc8qurssp5pqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyq9qrsgqcqpj7n3tcnerf357qjapjupmduwrryy3vdfk63jh45ssw86v34cxtdc9kk2n8hlhgs9f6uprj3eaxz54fwp3w2rkafhphh05llhtjdqp4sqptf062s',
  ],
] as const;
const BOLT11 =
  'lnbc2500u1pvjluezsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygspp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu9qrsgquk0rl77nj30yxdy8j9vdx85fkpmdla2087ne0xh8nhedh8w27kyke0lp53ut353s06fv3qfegext0eh0ymjpf39tuven09sam30g4vgpfna3rh';
const AMOUNTLESS_BOLT11 =
  'lnbc1pj48ugqdplf38yjgz8v9kx77fqv9kk7atww3kx2umnypex2emjv4ehx6t0dcsxv6tcw36hyegpp5pyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyysjzgfpyyssp5pg9q5zs2pg9q5zs2pg9q5zs2pg9q5zs2pg9q5zs2pg9q5zs2pg9q9qrsgqcqpjvcwldrltwv8ce6n00l8gl20vz5q3vu56hhmla07u39tmdy0ll6cs9crysytmdvugwrv2e6nwhfvlhd0mnjvskaefd43j9vdzjaggtygqe8yu0t';
const PAYMENT_HASH = '0001020304050607080900010203040506070809000102030405060708090102';

function jsonResponse(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: { 'content-type': 'application/json', ...(init?.headers ?? {}) },
  });
}

function config(overrides: Partial<GaloyConfig> = {}): GaloyConfig {
  return {
    apiKey: 'secret-api-key',
    baseUrl: 'https://galoy.test/graphql',
    provider: { id: 'flash', name: 'Flash' },
    wallet: { mode: 'explicit', id: 'wallet-jmd', currency: 'JMD' },
    invoiceOperations: {
      create: { kind: 'unsupported' },
      feeProbe: { kind: 'usd', denomination: 'usd-cents' },
    },
    payment: {
      response: 'status-only',
      acceptedStatuses: ['SUCCESS', 'PENDING', 'ALREADY_PAID'],
      statusMapping: {
        settled: ['SUCCESS', 'ALREADY_PAID'],
        pending: ['PENDING'],
      },
      proofUnavailableErrorCodes: ['PROOF_UNAVAILABLE'],
    },
    capabilities: {
      transactionLookup: false,
      transactionHistory: false,
      invoiceEvents: false,
      onchain: false,
    },
    permissions: 'configured',
    httpTimeout: 60,
    ...overrides,
  };
}

function bodyOf(init?: RequestInit): {
  query: string;
  variables?: Record<string, unknown>;
} {
  return JSON.parse(String(init?.body));
}

describe('createGaloyNode wallet and invoice behavior', () => {
  it('uses explicit operation configuration instead of inferring from wallet currency', async () => {
    const queries: string[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const body = bodyOf(init);
      queries.push(body.query);
      expect(body.variables).toMatchObject({ input: { walletId: 'wallet-jmd' } });
      return jsonResponse({
        data: {
          lnInvoiceCreate: {
            invoice: {
              paymentRequest: BOLT11,
              paymentHash: PAYMENT_HASH,
              satoshis: 21,
            },
            errors: [],
          },
        },
      });
    });

    const transaction = await createGaloyNode(
      config({
        invoiceOperations: {
          create: { kind: 'btc', denomination: 'sats' },
          feeProbe: { kind: 'btc', denomination: 'sats' },
        },
      }),
      { fetch: fetchMock }
    ).createInvoice({ amountMsats: 21_000 });

    expect(transaction.amountMsats).toBe(21_000);
    expect(queries).toHaveLength(1);
    expect(queries[0]).toContain('lnInvoiceCreate');
    expect(queries[0]).not.toContain('lnUsdInvoiceCreate');
    expect(queries[0]).not.toContain('query Me');
  });

  it('does not convert amountMsats into USD cents', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = createGaloyNode(
      config({
        invoiceOperations: {
          create: { kind: 'usd', denomination: 'usd-cents' },
          feeProbe: { kind: 'usd', denomination: 'usd-cents' },
        },
      }),
      { fetch: fetchMock }
    );

    await expect(node.createInvoice({ amountMsats: 21_000 })).rejects.toMatchObject({
      nwcCode: 'NOT_IMPLEMENTED',
      operation: 'make_invoice',
      message: expect.stringContaining('USD-cent'),
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('selects the requested wallet currency and reports NOT_FOUND when absent', async () => {
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse({
        data: {
          me: {
            defaultAccount: {
              wallets: [
                { id: 'btc', walletCurrency: 'BTC', balance: 10 },
                { id: 'jmd', walletCurrency: 'JMD', balance: 20 },
              ],
            },
          },
        },
      })
    );
    const jmdNode = createGaloyNode(config({ wallet: { mode: 'currency', currency: 'JMD' } }), {
      fetch: fetchMock,
    });
    await expect(jmdNode.getInfo()).resolves.toMatchObject({
      alias: 'Flash Node',
      sendBalanceMsat: 0,
      receiveBalanceMsat: 0,
    });

    const missingNode = createGaloyNode(config({ wallet: { mode: 'currency', currency: 'USD' } }), {
      fetch: fetchMock,
    });
    await expect(missingNode.getInfo()).rejects.toMatchObject({
      nwcCode: 'NOT_FOUND',
      provider: 'flash',
      message: expect.stringContaining('USD'),
    });
  });

  it('refreshes currency-wallet balances on each getInfo call', async () => {
    let balance = 1;
    const fetchMock = vi.fn<FetchLike>(async () =>
      jsonResponse({
        data: {
          me: {
            defaultAccount: {
              wallets: [{ id: 'btc', walletCurrency: 'BTC', balance: balance++ }],
            },
          },
        },
      })
    );
    const node = createGaloyNode(config({ wallet: { mode: 'currency', currency: 'BTC' } }), {
      fetch: fetchMock,
    });

    await expect(node.getInfo()).resolves.toMatchObject({ sendBalanceMsat: 1_000 });
    await expect(node.getInfo()).resolves.toMatchObject({ sendBalanceMsat: 2_000 });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('uses BTC create and fee-probe operations and converts BTC fees to msats', async () => {
    const queries: string[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      queries.push(query);
      if (query.includes('lnInvoiceFeeProbe')) {
        return jsonResponse({
          data: { lnInvoiceFeeProbe: { amount: 3, errors: [] } },
        });
      }
      return jsonResponse({
        data: { lnInvoicePaymentSend: { status: 'SUCCESS', errors: [] } },
      });
    });
    const node = createGaloyNode(
      config({
        wallet: { mode: 'explicit', id: 'wallet-btc', currency: 'BTC' },
        invoiceOperations: {
          create: { kind: 'btc', denomination: 'sats' },
          feeProbe: { kind: 'btc', denomination: 'sats' },
        },
        payment: { response: 'status-only', acceptedStatuses: ['SUCCESS'] },
      }),
      { fetch: fetchMock }
    );

    await expect(node.payInvoice({ invoice: BOLT11 })).resolves.toEqual({
      paymentHash: PAYMENT_HASH,
      preimage: '',
      feeMsats: 3_000,
    });
    expect(queries[0]).toContain('lnInvoiceFeeProbe');
    expect(queries[0]).not.toContain('lnUsdInvoiceFeeProbe');
  });

  it('uses lnUsd fee probes and never treats non-BTC fees as satoshis', async () => {
    const queries: string[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      queries.push(query);
      if (query.includes('lnUsdInvoiceFeeProbe')) {
        return jsonResponse({
          data: { lnUsdInvoiceFeeProbe: { amount: 99_999, errors: [] } },
        });
      }
      return jsonResponse({
        data: { lnInvoicePaymentSend: { status: 'SUCCESS', errors: [] } },
      });
    });

    await expect(
      createGaloyNode(config(), { fetch: fetchMock }).payInvoice({ invoice: BOLT11 })
    ).resolves.toMatchObject({ feeMsats: 0 });
    expect(queries[0]).toContain('lnUsdInvoiceFeeProbe');
  });
});

describe('createGaloyNode transport and payment modes', () => {
  it('forwards additional headers without allowing auth or content-type overrides', async () => {
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const headers = new Headers(init?.headers);
      expect(headers.get('x-flash-client-capabilities')).toBe('proofless');
      expect(headers.get('x-api-key')).toBe('secret-api-key');
      expect(headers.get('content-type')).toBe('application/json');
      return jsonResponse({
        data: {
          lnInvoiceCreate: {
            invoice: {
              paymentRequest: BOLT11,
              paymentHash: PAYMENT_HASH,
              satoshis: 1,
            },
            errors: [],
          },
        },
      });
    });

    const node = createGaloyNode(
      config({
        invoiceOperations: {
          create: { kind: 'btc', denomination: 'sats' },
          feeProbe: { kind: 'btc', denomination: 'sats' },
        },
        additionalHeaders: {
          'x-flash-client-capabilities': 'proofless',
          'X-API-Key': 'attacker-key',
          'Content-Type': 'text/plain',
        },
      }),
      { fetch: fetchMock }
    );
    await node.createInvoice({ amountMsats: 1_000 });
  });

  it('requests and returns a preimage only in transaction-with-preimage mode', async () => {
    let paymentQuery = '';
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      if (query.includes('FeeProbe')) {
        return jsonResponse({
          data: { lnInvoiceFeeProbe: { amount: 1, errors: [] } },
        });
      }
      paymentQuery = query;
      return jsonResponse({
        data: {
          lnInvoicePaymentSend: {
            status: 'SUCCESS',
            errors: [],
            transaction: { settlementVia: { preImage: 'provider-preimage' } },
          },
        },
      });
    });
    const node = createGaloyNode(
      config({
        wallet: { mode: 'explicit', id: 'btc', currency: 'BTC' },
        invoiceOperations: {
          create: { kind: 'btc', denomination: 'sats' },
          feeProbe: { kind: 'btc', denomination: 'sats' },
        },
        payment: {
          response: 'transaction-with-preimage',
          acceptedStatuses: ['SUCCESS'],
        },
      }),
      { fetch: fetchMock }
    );

    await expect(node.payInvoice({ invoice: BOLT11 })).resolves.toMatchObject({
      paymentHash: PAYMENT_HASH,
      preimage: 'provider-preimage',
    });
    expect(paymentQuery).toContain('transaction {');
    expect(paymentQuery).toContain('SettlementViaIntraLedger');
  });

  it.each(ONE_SAT_BOLT11S)(
    'accepts a valid 1-sat %s invoice whose encoded amount begins with 1',
    async (_network, invoice) => {
      const queries: string[] = [];
      const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
        const query = bodyOf(init).query;
        queries.push(query);
        return query.includes('lnInvoiceFeeProbe')
          ? jsonResponse({ data: { lnInvoiceFeeProbe: { amount: 1, errors: [] } } })
          : jsonResponse({
              data: { lnInvoicePaymentSend: { status: 'SUCCESS', errors: [] } },
            });
      });
      const node = createGaloyNode(
        config({
          wallet: { mode: 'explicit', id: 'btc', currency: 'BTC' },
          invoiceOperations: {
            create: { kind: 'btc', denomination: 'sats' },
            feeProbe: { kind: 'btc', denomination: 'sats' },
          },
          payment: { response: 'status-only', acceptedStatuses: ['SUCCESS'] },
        }),
        { fetch: fetchMock }
      );

      expect(decode(invoice).amountMsats).toBe(1_000);
      await expect(node.payInvoice({ invoice })).resolves.toMatchObject({
        paymentHash: '07'.repeat(32),
        preimage: '',
        feeMsats: 1_000,
      });
      expect(queries).toHaveLength(2);
      expect(queries[0]).toContain('lnInvoiceFeeProbe');
      expect(queries[1]).toContain('lnInvoicePaymentSend');
    }
  );

  it.each([undefined, 1_000])(
    'rejects a genuinely amountless invoice before any request (amountMsats: %s)',
    async (amountMsats) => {
      const fetchMock = vi.fn<FetchLike>();
      const node = createGaloyNode(config(), { fetch: fetchMock });

      await expect(
        node.payInvoice({ invoice: AMOUNTLESS_BOLT11, amountMsats })
      ).rejects.toMatchObject({
        code: 'InvalidInput',
        message: expect.stringContaining(
          "Flash cannot pay amountless BOLT11 invoices because Galoy's payment mutation has no amount field."
        ),
      });
      expect(fetchMock).not.toHaveBeenCalled();
    }
  );

  it('lets the provider handle malformed invoices instead of calling them amountless', async () => {
    const queries: string[] = [];
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      queries.push(query);
      return query.includes('lnUsdInvoiceFeeProbe')
        ? jsonResponse({ data: { lnUsdInvoiceFeeProbe: { amount: 0, errors: [] } } })
        : jsonResponse({
            data: {
              lnInvoicePaymentSend: {
                status: 'FAILURE',
                errors: [{ code: 'INVALID_INVOICE', message: 'Invalid invoice' }],
              },
            },
          });
    });
    const node = createGaloyNode(config(), { fetch: fetchMock });

    await expect(node.payInvoice({ invoice: 'not-a-bolt11-invoice' })).rejects.toMatchObject({
      nwcCode: 'PAYMENT_FAILED',
      provider: 'flash',
      providerCode: 'FAILURE',
      providerMessage: 'Invalid invoice',
    });
    expect(queries).toHaveLength(2);
    expect(queries[0]).toContain('lnUsdInvoiceFeeProbe');
    expect(queries[1]).toContain('lnInvoicePaymentSend');
  });

  it('normalizes missing payment and on-chain mutation payloads', async () => {
    const paymentFetch = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      return query.includes('FeeProbe')
        ? jsonResponse({ data: { lnInvoiceFeeProbe: { amount: 1, errors: [] } } })
        : jsonResponse({ data: {} });
    });
    const btcConfig = config({
      wallet: { mode: 'explicit', id: 'btc', currency: 'BTC' },
      invoiceOperations: {
        create: { kind: 'btc', denomination: 'sats' },
        feeProbe: { kind: 'btc', denomination: 'sats' },
      },
      payment: { response: 'status-only', acceptedStatuses: ['SUCCESS'] },
      capabilities: { ...config().capabilities, onchain: true },
    });
    const node = createGaloyNode(btcConfig, { fetch: paymentFetch });

    await expect(node.payInvoice({ invoice: BOLT11 })).rejects.toMatchObject({
      nwcCode: 'INTERNAL',
      provider: 'flash',
      message: expect.stringContaining('lnInvoicePaymentSend'),
    });

    const onchainFetch = vi.fn<FetchLike>(async () => jsonResponse({ data: {} }));
    const onchainNode = createGaloyNode(btcConfig, { fetch: onchainFetch });
    await expect(
      onchainNode.prepareOnchainTransaction({
        address: 'bc1qexample',
        amountSats: 10_000,
      })
    ).rejects.toMatchObject({
      nwcCode: 'INTERNAL',
      operation: 'prepare_onchain_transaction',
      provider: 'flash',
      message: expect.stringContaining('onChainTxFee'),
    });
    await expect(
      onchainNode.payOnchain({
        address: 'bc1qexample',
        amountSats: 10_000,
        feeSats: 1_000,
        feePayer: 'sender',
        fee: { type: 'default' },
      })
    ).rejects.toMatchObject({
      nwcCode: 'INTERNAL',
      operation: 'pay_onchain',
      provider: 'flash',
      message: expect.stringContaining('onChainPaymentSend'),
    });
  });

  it.each(['SUCCESS', 'PENDING', 'ALREADY_PAID'])(
    'accepts configured proofless status %s and omits transaction from the query',
    async (status) => {
      let paymentQuery = '';
      const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
        const query = bodyOf(init).query;
        if (query.includes('FeeProbe')) {
          return jsonResponse({
            data: { lnUsdInvoiceFeeProbe: { amount: 20, errors: [] } },
          });
        }
        paymentQuery = query;
        return jsonResponse({
          data: { lnInvoicePaymentSend: { status, errors: [] } },
        });
      });

      const node = createGaloyNode(config(), { fetch: fetchMock });
      await expect(node.payInvoiceWithStatus({ invoice: BOLT11 })).resolves.toEqual({
        payment: {
          paymentHash: PAYMENT_HASH,
          preimage: '',
          feeMsats: 0,
        },
        state: status === 'PENDING' ? 'pending' : 'settled',
        providerStatus: status,
      });
      expect(paymentQuery).not.toMatch(/\btransaction\s*\{/);
    }
  );

  it('does not turn an accepted status into a failure when no proof is available', async () => {
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      if (query.includes('FeeProbe')) {
        return jsonResponse({
          data: { lnUsdInvoiceFeeProbe: { amount: 20, errors: [] } },
        });
      }
      return jsonResponse({
        data: {
          lnInvoicePaymentSend: {
            status: 'PENDING',
            errors: [{ code: 'PROOF_UNAVAILABLE', message: 'No proof is exposed' }],
          },
        },
      });
    });

    const node = createGaloyNode(config(), { fetch: fetchMock });
    await expect(node.payInvoiceWithStatus({ invoice: BOLT11 })).resolves.toEqual({
      payment: {
        paymentHash: PAYMENT_HASH,
        preimage: '',
        feeMsats: 0,
      },
      state: 'pending',
      providerStatus: 'PENDING',
    });
  });

  it('surfaces unexpected payload errors even when the status is accepted', async () => {
    const sensitiveProviderMessage = [
      `payment_request=${BOLT11}`,
      `payment_hash=${PAYMENT_HASH}`,
      'payment_secret=payment-secret',
      'preimage=provider-preimage',
      'access_token=access-token',
      'api_key=secret-api-key',
    ].join(' ');
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      if (query.includes('FeeProbe')) {
        return jsonResponse({
          data: { lnUsdInvoiceFeeProbe: { amount: 20, errors: [] } },
        });
      }
      return jsonResponse({
        data: {
          lnInvoicePaymentSend: {
            status: 'SUCCESS',
            errors: [{ code: 'UNEXPECTED_PROVIDER_ERROR', message: sensitiveProviderMessage }],
          },
        },
      });
    });

    try {
      await createGaloyNode(config(), { fetch: fetchMock }).payInvoice({ invoice: BOLT11 });
      throw new Error('expected payInvoice to fail');
    } catch (error) {
      expect(error).toMatchObject({
        nwcCode: 'PAYMENT_FAILED',
        provider: 'flash',
        providerCode: 'UNEXPECTED_PROVIDER_ERROR',
      });
      const serialized = JSON.stringify(error);
      for (const secret of [
        BOLT11,
        PAYMENT_HASH,
        'payment-secret',
        'provider-preimage',
        'access-token',
        'secret-api-key',
      ]) {
        expect(serialized).not.toContain(secret);
      }
    }
  });

  it('uses configured provider metadata for rejected statuses and redacts secrets', async () => {
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const query = bodyOf(init).query;
      if (query.includes('FeeProbe')) {
        return jsonResponse({
          data: { lnUsdInvoiceFeeProbe: { amount: 0, errors: [] } },
        });
      }
      return jsonResponse({
        data: { lnInvoicePaymentSend: { status: 'DECLINED', errors: [] } },
      });
    });
    const node = createGaloyNode(
      config({ payment: { response: 'status-only', acceptedStatuses: ['SUCCESS'] } }),
      { fetch: fetchMock }
    );

    await expect(node.payInvoice({ invoice: BOLT11 })).rejects.toMatchObject({
      provider: 'flash',
      providerCode: 'DECLINED',
      providerMessage: 'DECLINED',
      message: expect.stringContaining('Flash'),
    });

    const secretFetch = vi.fn<FetchLike>(async () =>
      jsonResponse(
        {
          message: 'bad request',
          paymentRequest: BOLT11,
          preImage: 'secret-preimage',
          apiKey: 'secret-api-key',
        },
        { status: 400 }
      )
    );
    try {
      await createGaloyNode(config({ wallet: { mode: 'currency', currency: 'JMD' } }), {
        fetch: secretFetch,
      }).getInfo();
      throw new Error('expected getInfo to fail');
    } catch (error) {
      expect(error).toBeInstanceOf(NwcError);
      expect(JSON.stringify(error)).not.toContain(BOLT11);
      expect(JSON.stringify(error)).not.toContain('secret-preimage');
      expect(JSON.stringify(error)).not.toContain('secret-api-key');
    }
  });
});

describe('createGaloyNode capabilities and compatibility', () => {
  it('reports configured permissions and blocks disabled transaction methods without queries', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = createGaloyNode(config(), { fetch: fetchMock });

    await expect(node.getPermissions()).resolves.toMatchObject({
      getInfo: true,
      createInvoice: false,
      payInvoice: true,
      decode: true,
      lookupInvoice: false,
      listTransactions: false,
      onInvoiceEvents: false,
    });
    await expect(node.lookupInvoice({ paymentHash: PAYMENT_HASH })).rejects.toMatchObject({
      nwcCode: 'NOT_IMPLEMENTED',
      provider: 'flash',
    });
    await expect(node.listTransactions({ from: 0, limit: 10 })).rejects.toMatchObject({
      nwcCode: 'NOT_IMPLEMENTED',
      provider: 'flash',
    });
    await expect(
      node.onInvoiceEvents(
        { paymentHash: PAYMENT_HASH, pollingDelaySec: 1, maxPollingSec: 1 },
        () => {}
      )
    ).rejects.toMatchObject({
      nwcCode: 'NOT_IMPLEMENTED',
      provider: 'flash',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('does not expose on-chain methods for non-BTC wallets', async () => {
    const fetchMock = vi.fn<FetchLike>();
    const node = createGaloyNode(
      config({ capabilities: { ...config().capabilities, onchain: true } }),
      { fetch: fetchMock }
    );
    await expect(
      node.prepareOnchainTransaction({ address: 'address', amountSats: 1 })
    ).rejects.toMatchObject({
      nwcCode: 'NOT_IMPLEMENTED',
      provider: 'flash',
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('supports kind galoy and keeps kind blink behavior through the generic transport', async () => {
    const galoyFetch = vi.fn<FetchLike>(async () =>
      jsonResponse({
        data: {
          lnInvoiceCreate: {
            invoice: {
              paymentRequest: BOLT11,
              paymentHash: PAYMENT_HASH,
              satoshis: 1,
            },
            errors: [],
          },
        },
      })
    );
    const galoy = createNode(
      {
        kind: 'galoy',
        config: config({
          invoiceOperations: {
            create: { kind: 'btc', denomination: 'sats' },
            feeProbe: { kind: 'btc', denomination: 'sats' },
          },
        }),
      },
      { fetch: galoyFetch }
    );
    await expect(galoy.createInvoice({ amountMsats: 1_000 })).resolves.toMatchObject({
      amountMsats: 1_000,
    });

    const blinkFetch = vi.fn<FetchLike>(async (_input, init) => {
      const body = bodyOf(init);
      expect(new Headers(init?.headers).get('x-api-key')).toBe('blink-key');
      expect(body.query).toContain('query Me');
      return jsonResponse({
        data: {
          me: {
            defaultAccount: {
              wallets: [{ id: 'btc', walletCurrency: 'BTC', balance: 2 }],
            },
          },
        },
      });
    });
    const blink = createNode(
      { kind: 'blink', config: { apiKey: 'blink-key' } },
      { fetch: blinkFetch }
    );
    await expect(blink.getInfo()).resolves.toMatchObject({
      alias: 'Blink Node',
      sendBalanceMsat: 2_000,
    });
  });

  it('bounds transaction lookup to 1,000 scanned provider records', async () => {
    let pageNumber = 0;
    const fetchMock = vi.fn<FetchLike>(async (_input, init) => {
      const body = bodyOf(init);
      const first = Number(body.variables?.first);
      const currentPage = pageNumber++;
      expect(first).toBe(100);
      return jsonResponse({
        data: {
          me: {
            defaultAccount: {
              transactions: {
                edges: Array.from({ length: first }, (_, index) => ({
                  cursor: `cursor-${currentPage}-${index}`,
                  node: {
                    id: `transaction-${currentPage}-${index}`,
                    createdAt: 1,
                    direction: 'RECEIVE',
                    status: 'SUCCESS',
                    settlementAmount: 1,
                    settlementCurrency: 'BTC',
                    initiationVia: {
                      __typename: 'InitiationViaLn',
                      paymentHash: `not-${PAYMENT_HASH}`,
                    },
                  },
                })),
                pageInfo: {
                  hasNextPage: true,
                  endCursor: `page-${currentPage + 1}`,
                },
              },
            },
          },
        },
      });
    });
    const node = createGaloyNode(
      config({
        capabilities: { ...config().capabilities, transactionLookup: true },
      }),
      { fetch: fetchMock }
    );

    await expect(node.lookupInvoice({ paymentHash: PAYMENT_HASH })).rejects.toMatchObject({
      nwcCode: 'NOT_FOUND',
    });
    expect(fetchMock).toHaveBeenCalledTimes(10);
  });
});
