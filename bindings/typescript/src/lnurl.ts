import { bech32 } from '@scure/base';
import { LniError } from './errors.js';
import type { FetchLike, PaymentInfo } from './types.js';
import { resolveFetch, requestJson } from './internal/http.js';

export type PaymentDestinationType = 'bolt11' | 'bolt12' | 'lnurl' | 'lightning_address';

interface LnurlPayResponse {
  callback: string;
  maxSendable: number;
  minSendable: number;
  metadata: string;
  tag: string;
  allowsNostr?: boolean;
  nostrPubkey?: string;
}

interface LnurlInvoiceResponse {
  pr: string;
}

interface LnurlErrorResponse {
  status: string;
  reason: string;
}

export function detectPaymentType(destination: string): PaymentDestinationType {
  const input = destination.trim();
  const lower = input.toLowerCase();

  if (input.includes('@') && !lower.startsWith('lnurl')) {
    return 'lightning_address';
  }
  if (lower.startsWith('lnbc') || lower.startsWith('lntb') || lower.startsWith('lntbs')) {
    return 'bolt11';
  }
  if (lower.startsWith('lno1')) {
    return 'bolt12';
  }
  if (lower.startsWith('lnurl1')) {
    return 'lnurl';
  }

  throw new LniError(
    'InvalidInput',
    'Unknown payment destination format. Expected BOLT11, BOLT12, LNURL, or Lightning Address.',
  );
}

export function needsResolution(destination: string): boolean {
  const normalized = destination.trim().toLowerCase();
  return (normalized.includes('@') && !normalized.startsWith('lnurl')) || normalized.startsWith('lnurl1');
}

export function lightningAddressToUrl(user: string, domain: string): string {
  return `https://${domain}/.well-known/lnurlp/${user}`;
}

export function decodeLnurl(lnurl: string): string {
  try {
    const decoded = bech32.decode(lnurl.toLowerCase() as `${string}1${string}`, Number.MAX_SAFE_INTEGER);
    if (decoded.prefix !== 'lnurl') {
      throw new LniError('InvalidInput', "LNURL must use the 'lnurl' prefix.");
    }

    const bytes = Uint8Array.from(bech32.fromWords(decoded.words));
    return new TextDecoder().decode(bytes);
  } catch (error) {
    if (error instanceof LniError) {
      throw error;
    }
    throw new LniError('InvalidInput', `Invalid LNURL encoding: ${(error as Error)?.message ?? 'unknown error'}`);
  }
}

async function fetchLnurlPay(url: string, fetchFn: FetchLike): Promise<LnurlPayResponse> {
  const payload = await requestJson<LnurlPayResponse | LnurlErrorResponse>(fetchFn, url, {
    method: 'GET',
    headers: {
      accept: 'application/json',
    },
    timeoutMs: 30_000,
  });

  const maybeError = payload as LnurlErrorResponse;
  if (maybeError?.status === 'ERROR') {
    throw new LniError('LnurlError', maybeError.reason);
  }

  return payload as LnurlPayResponse;
}

async function requestInvoice(callbackUrl: string, amountMsats: number, fetchFn: FetchLike): Promise<string> {
  const callback = new URL(callbackUrl);
  callback.searchParams.set('amount', String(amountMsats));

  const response = await requestJson<LnurlInvoiceResponse | LnurlErrorResponse>(fetchFn, callback.toString(), {
    method: 'GET',
    headers: {
      accept: 'application/json',
    },
    timeoutMs: 30_000,
  });

  const maybeError = response as LnurlErrorResponse;
  if (maybeError.status === 'ERROR') {
    throw new LniError('LnurlError', maybeError.reason);
  }

  const invoiceResponse = response as LnurlInvoiceResponse;
  if (!invoiceResponse.pr) {
    throw new LniError('Json', 'Invalid LNURL invoice response: missing pr field');
  }

  return invoiceResponse.pr;
}

function parseLightningAddress(input: string): { user: string; domain: string } {
  const parts = input.split('@');
  if (parts.length !== 2 || !parts[0] || !parts[1]) {
    throw new LniError('InvalidInput', 'Invalid Lightning Address format.');
  }
  return { user: parts[0], domain: parts[1] };
}

function assertAmountRange(amountMsats: number, minSendable: number, maxSendable: number): void {
  if (amountMsats < minSendable) {
    throw new LniError('InvalidInput', `Amount ${amountMsats} msats is below minimum ${minSendable} msats`);
  }
  if (amountMsats > maxSendable) {
    throw new LniError('InvalidInput', `Amount ${amountMsats} msats exceeds maximum ${maxSendable} msats`);
  }
}

async function resolveViaLnurlPay(url: string, amountMsats: number, fetchFn: FetchLike): Promise<string> {
  const lnurlPay = await fetchLnurlPay(url, fetchFn);
  assertAmountRange(amountMsats, lnurlPay.minSendable, lnurlPay.maxSendable);
  return requestInvoice(lnurlPay.callback, amountMsats, fetchFn);
}

export async function resolveToBolt11(
  destination: string,
  amountMsats?: number,
  options?: { fetch?: FetchLike },
): Promise<string> {
  const fetchFn = resolveFetch(options?.fetch);
  const destinationType = detectPaymentType(destination);

  if (destinationType === 'bolt11') {
    return destination.trim();
  }

  if (destinationType === 'bolt12') {
    throw new LniError('InvalidInput', 'BOLT12 offers should be paid via payOffer.');
  }

  if (amountMsats === undefined || amountMsats === null) {
    throw new LniError('InvalidInput', 'LNURL and Lightning Address resolution requires amountMsats.');
  }

  if (destinationType === 'lightning_address') {
    const { user, domain } = parseLightningAddress(destination.trim());
    return resolveViaLnurlPay(lightningAddressToUrl(user, domain), amountMsats, fetchFn);
  }

  const lnurl = decodeLnurl(destination.trim());
  return resolveViaLnurlPay(lnurl, amountMsats, fetchFn);
}

export async function getPaymentInfo(
  destination: string,
  amountMsats?: number,
  options?: { fetch?: FetchLike },
): Promise<PaymentInfo> {
  const fetchFn = resolveFetch(options?.fetch);
  const destinationType = detectPaymentType(destination);

  if (destinationType === 'bolt11' || destinationType === 'bolt12') {
    return {
      destinationType,
      destination,
      amountMsats,
    };
  }

  if (destinationType === 'lightning_address') {
    const { user, domain } = parseLightningAddress(destination.trim());
    const lnurlPay = await fetchLnurlPay(lightningAddressToUrl(user, domain), fetchFn);
    return {
      destinationType,
      destination,
      amountMsats,
      minSendableMsats: lnurlPay.minSendable,
      maxSendableMsats: lnurlPay.maxSendable,
      description: lnurlPay.metadata,
    };
  }

  const lnurl = decodeLnurl(destination.trim());
  const lnurlPay = await fetchLnurlPay(lnurl, fetchFn);
  return {
    destinationType,
    destination,
    amountMsats,
    minSendableMsats: lnurlPay.minSendable,
    maxSendableMsats: lnurlPay.maxSendable,
    description: lnurlPay.metadata,
  };
}
