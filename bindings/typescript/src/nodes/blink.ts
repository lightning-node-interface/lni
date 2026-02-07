import { LniError } from '../errors.js';
import { requestJson, resolveFetch, toTimeoutMs } from '../internal/http.js';
import { pollInvoiceEvents } from '../internal/polling.js';
import { emptyNodeInfo, emptyTransaction, matchesSearch, satsToMsats } from '../internal/transform.js';
import { InvoiceType, type BlinkConfig, type CreateInvoiceParams, type CreateOfferParams, type InvoiceEventCallback, type LightningNode, type ListTransactionsParams, type LookupInvoiceParams, type NodeInfo, type NodeRequestOptions, type Offer, type OnInvoiceEventParams, type PayInvoiceParams, type PayInvoiceResponse, type Transaction } from '../types.js';

interface GraphQLError {
  message: string;
}

interface GraphQLResponse<T> {
  data?: T;
  errors?: GraphQLError[];
}

interface BlinkMeQuery {
  me: {
    defaultAccount: {
      wallets: BlinkWallet[];
    };
  };
}

interface BlinkWallet {
  id: string;
  walletCurrency: string;
  balance: number;
}

interface BlinkInvoiceCreateResponse {
  lnInvoiceCreate: {
    invoice?: {
      paymentRequest: string;
      paymentHash: string;
      satoshis: number;
    };
    errors?: GraphQLError[];
  };
}

interface BlinkFeeProbeResponse {
  lnInvoiceFeeProbe: {
    amount?: number;
    errors?: GraphQLError[];
  };
}

interface BlinkPaymentSendResponse {
  lnInvoicePaymentSend: {
    status: string;
    errors?: GraphQLError[];
  };
}

interface BlinkTransactionsQuery {
  me: {
    defaultAccount: {
      transactions: {
        edges: Array<{
          cursor: string;
          node: {
            id: string;
            createdAt: number;
            direction: 'SEND' | 'RECEIVE';
            status: string;
            memo?: string;
            settlementAmount?: number;
            settlementCurrency?: string;
            settlementFee?: number;
            initiationVia?: {
              __typename: string;
              paymentHash?: string;
            };
            settlementVia?: {
              __typename: string;
              preImage?: string;
            };
          };
        }>;
        pageInfo: {
          hasNextPage: boolean;
          endCursor?: string | null;
        };
      };
    };
  };
}

type BlinkTransactionNode = BlinkTransactionsQuery['me']['defaultAccount']['transactions']['edges'][number]['node'];

interface BlinkTransactionsPage {
  transactions: Transaction[];
  nextCursor: string | null;
}

export class BlinkNode implements LightningNode {
  private readonly fetchFn;
  private readonly timeoutMs?: number;
  private readonly baseUrl: string;
  private cachedWalletId?: string;
  private static readonly MAX_TRANSACTION_FETCH = 1000;
  private static readonly DEFAULT_PAGE_SIZE = 100;

  private static readonly ME_QUERY = `
    query Me {
      me {
        defaultAccount {
          wallets {
            id
            walletCurrency
            balance
          }
        }
      }
    }
  `;

  private static readonly TRANSACTIONS_QUERY = `
    query TransactionsQuery($first: Int, $after: String) {
      me {
        defaultAccount {
          transactions(first: $first, after: $after) {
            edges {
              cursor
              node {
                id
                createdAt
                direction
                status
                memo
                settlementAmount
                settlementCurrency
                settlementFee
                initiationVia {
                  __typename
                  ... on InitiationViaLn {
                    paymentHash
                  }
                }
                settlementVia {
                  __typename
                  ... on SettlementViaLn {
                    preImage
                  }
                }
              }
            }
            pageInfo {
              hasNextPage
              endCursor
            }
          }
        }
      }
    }
  `;

  constructor(private readonly config: BlinkConfig, options: NodeRequestOptions = {}) {
    this.fetchFn = resolveFetch(options.fetch);
    this.timeoutMs = toTimeoutMs(config.httpTimeout);
    this.baseUrl = config.baseUrl ?? 'https://api.blink.sv/graphql';
  }

  private headers(extra?: HeadersInit): HeadersInit {
    return {
      'x-api-key': this.config.apiKey,
      'content-type': 'application/json',
      ...(extra ?? {}),
    };
  }

  private async gql<T>(query: string, variables?: Record<string, unknown>): Promise<T> {
    const payload = await requestJson<GraphQLResponse<T>>(this.fetchFn, this.baseUrl, {
      method: 'POST',
      headers: this.headers(),
      json: {
        query,
        variables,
      },
      timeoutMs: this.timeoutMs,
    });

    if (payload.errors?.length) {
      throw new LniError('Api', payload.errors.map((error) => error.message).join(', '));
    }

    if (!payload.data) {
      throw new LniError('Json', 'No data in Blink GraphQL response.');
    }

    return payload.data;
  }

  private async getBtcWallet(): Promise<BlinkWallet> {
    const response = await this.gql<BlinkMeQuery>(BlinkNode.ME_QUERY);
    const wallet = response.me.defaultAccount.wallets.find((item) => item.walletCurrency === 'BTC');

    if (!wallet) {
      throw new LniError('Api', 'No BTC wallet found in Blink account.');
    }

    this.cachedWalletId = wallet.id;
    return wallet;
  }

  private async getBtcWalletId(): Promise<string> {
    if (this.cachedWalletId) {
      return this.cachedWalletId;
    }

    const wallet = await this.getBtcWallet();
    this.cachedWalletId = wallet.id;
    return wallet.id;
  }

  async getInfo(): Promise<NodeInfo> {
    const wallet = await this.getBtcWallet();
    const sats = wallet.balance;

    return emptyNodeInfo({
      alias: 'Blink Node',
      network: 'mainnet',
      sendBalanceMsat: satsToMsats(sats),
      receiveBalanceMsat: satsToMsats(sats),
    });
  }

  async createInvoice(params: CreateInvoiceParams): Promise<Transaction> {
    if ((params.invoiceType ?? InvoiceType.Bolt11) !== InvoiceType.Bolt11) {
      throw new LniError('Api', 'Bolt12 is not implemented for BlinkNode.');
    }

    const walletId = await this.getBtcWalletId();

    const query = `
      mutation LnInvoiceCreate($input: LnInvoiceCreateInput!) {
        lnInvoiceCreate(input: $input) {
          invoice {
            paymentRequest
            paymentHash
            satoshis
          }
          errors {
            message
          }
        }
      }
    `;

    const response = await this.gql<BlinkInvoiceCreateResponse>(query, {
      input: {
        amount: Math.floor((params.amountMsats ?? 0) / 1000),
        walletId,
        memo: params.description,
      },
    });

    if (response.lnInvoiceCreate.errors?.length) {
      throw new LniError('Api', response.lnInvoiceCreate.errors.map((error) => error.message).join(', '));
    }

    const invoice = response.lnInvoiceCreate.invoice;
    if (!invoice) {
      throw new LniError('Json', 'No invoice returned from Blink invoice creation.');
    }

    return emptyTransaction({
      type: 'incoming',
      invoice: invoice.paymentRequest,
      paymentHash: invoice.paymentHash,
      amountMsats: satsToMsats(invoice.satoshis),
      createdAt: Math.floor(Date.now() / 1000),
      description: params.description ?? '',
      descriptionHash: params.descriptionHash ?? '',
      payerNote: '',
      externalId: '',
    });
  }

  async payInvoice(params: PayInvoiceParams): Promise<PayInvoiceResponse> {
    const walletId = await this.getBtcWalletId();

    const feeProbe = await this.gql<BlinkFeeProbeResponse>(
      `
      mutation lnInvoiceFeeProbe($input: LnInvoiceFeeProbeInput!) {
        lnInvoiceFeeProbe(input: $input) {
          errors {
            message
          }
          amount
        }
      }
      `,
      {
        input: {
          paymentRequest: params.invoice,
          walletId,
        },
      },
    );

    if (feeProbe.lnInvoiceFeeProbe.errors?.length) {
      throw new LniError('Api', feeProbe.lnInvoiceFeeProbe.errors.map((error) => error.message).join(', '));
    }

    const payment = await this.gql<BlinkPaymentSendResponse>(
      `
      mutation LnInvoicePaymentSend($input: LnInvoicePaymentInput!) {
        lnInvoicePaymentSend(input: $input) {
          status
          errors {
            message
          }
        }
      }
      `,
      {
        input: {
          paymentRequest: params.invoice,
          walletId,
        },
      },
    );

    if (payment.lnInvoicePaymentSend.errors?.length) {
      throw new LniError('Api', payment.lnInvoicePaymentSend.errors.map((error) => error.message).join(', '));
    }

    if (payment.lnInvoicePaymentSend.status !== 'SUCCESS') {
      throw new LniError('Api', `Blink payment failed with status ${payment.lnInvoicePaymentSend.status}`);
    }

    return {
      paymentHash: '',
      preimage: '',
      feeMsats: satsToMsats(feeProbe.lnInvoiceFeeProbe.amount ?? 0),
    };
  }

  async createOffer(_params: CreateOfferParams): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for BlinkNode.');
  }

  async getOffer(_search?: string): Promise<Offer> {
    throw new LniError('Api', 'Bolt12 is not implemented for BlinkNode.');
  }

  async listOffers(_search?: string): Promise<Offer[]> {
    throw new LniError('Api', 'Bolt12 is not implemented for BlinkNode.');
  }

  async payOffer(_offer: string, _amountMsats: number, _payerNote?: string): Promise<PayInvoiceResponse> {
    throw new LniError('Api', 'Bolt12 is not implemented for BlinkNode.');
  }

  private mapTransaction(node: BlinkTransactionNode): Transaction {
    const paymentHash =
      node.initiationVia?.__typename === 'InitiationViaLn'
        ? (node.initiationVia.paymentHash ?? '')
        : '';
    const preimage =
      node.settlementVia?.__typename === 'SettlementViaLn' ? (node.settlementVia.preImage ?? '') : '';

    const amountMsats = node.settlementCurrency === 'BTC' ? satsToMsats(Math.abs(node.settlementAmount ?? 0)) : 0;
    const feeMsats = node.settlementCurrency === 'BTC' ? satsToMsats(Math.abs(node.settlementFee ?? 0)) : 0;

    return emptyTransaction({
      type: node.direction === 'SEND' ? 'outgoing' : 'incoming',
      paymentHash,
      preimage,
      amountMsats,
      feesPaid: feeMsats,
      createdAt: node.createdAt,
      settledAt: node.status === 'SUCCESS' ? node.createdAt : 0,
      description: node.memo ?? '',
      descriptionHash: '',
      payerNote: '',
      externalId: node.id,
    });
  }

  private async listTransactionsPage(args: {
    first: number;
    after?: string | null;
    paymentHash?: string;
    search?: string;
  }): Promise<BlinkTransactionsPage> {
    const response: BlinkTransactionsQuery = await this.gql<BlinkTransactionsQuery>(BlinkNode.TRANSACTIONS_QUERY, {
      first: Math.max(args.first, 1),
      after: args.after ?? null,
    });

    const page: BlinkTransactionsQuery['me']['defaultAccount']['transactions'] =
      response.me.defaultAccount.transactions;
    const edges = page.edges;
    const transactions = edges
      .map(({ node }) => this.mapTransaction(node))
      .filter((tx) => {
        if (args.paymentHash && tx.paymentHash !== args.paymentHash) {
          return false;
        }
        return matchesSearch(tx, args.search);
      });

    if (!page.pageInfo.hasNextPage) {
      return {
        transactions,
        nextCursor: null,
      };
    }

    const nextCursor: string | null = page.pageInfo.endCursor ?? edges[edges.length - 1]?.cursor ?? null;
    return {
      transactions,
      nextCursor: nextCursor && nextCursor !== args.after ? nextCursor : null,
    };
  }

  async lookupInvoice(params: LookupInvoiceParams): Promise<Transaction> {
    if (!params.paymentHash) {
      throw new LniError('InvalidInput', 'lookupInvoice requires paymentHash for BlinkNode.');
    }

    let after: string | null = null;

    while (true) {
      const page = await this.listTransactionsPage({
        first: 100,
        after,
        paymentHash: params.paymentHash,
        search: params.search,
      });

      const match = page.transactions.find((tx) => tx.paymentHash === params.paymentHash);
      if (match) {
        return match;
      }

      if (!page.nextCursor) {
        break;
      }

      after = page.nextCursor;
    }

    throw new LniError('Api', `Transaction not found for payment hash: ${params.paymentHash}`);
  }

  async listTransactions(params: ListTransactionsParams): Promise<Transaction[]> {
    const limit =
      params.limit > 0
        ? params.limit
        : Math.min(BlinkNode.MAX_TRANSACTION_FETCH, BlinkNode.DEFAULT_PAGE_SIZE * 10);
    const from = Math.max(params.from, 0);
    const pageSize = Math.max(Math.min(limit, BlinkNode.DEFAULT_PAGE_SIZE), 1);

    let after: string | null = null;
    let skipped = 0;
    const transactions: Transaction[] = [];

    while (transactions.length < limit) {
      const page = await this.listTransactionsPage({
        first: pageSize,
        after,
        paymentHash: params.paymentHash,
        search: params.search,
      });
      if (!page.transactions.length && !page.nextCursor) {
        break;
      }

      for (const tx of page.transactions) {
        if (skipped < from) {
          skipped += 1;
          continue;
        }

        transactions.push(tx);
        if (transactions.length >= limit) {
          break;
        }
      }

      if (!page.nextCursor) {
        break;
      }

      after = page.nextCursor;
    }

    return transactions;
  }

  async decode(str: string): Promise<string> {
    return str;
  }

  async onInvoiceEvents(params: OnInvoiceEventParams, callback: InvoiceEventCallback): Promise<void> {
    await pollInvoiceEvents({
      params,
      callback,
      lookup: () => this.lookupInvoice({ paymentHash: params.paymentHash, search: params.search }),
    });
  }
}
