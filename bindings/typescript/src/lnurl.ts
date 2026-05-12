import { bech32 } from '@scure/base';
import { decode as decodeBolt11 } from 'light-bolt11-decoder';
import { LniError } from './errors.js';
import type { FetchLike, PaymentInfo } from './types.js';
import { resolveFetch, requestJson } from './internal/http.js';

export type PaymentDestinationType = 'bolt11' | 'bolt12' | 'lnurl' | 'lightning_address';

export interface LnurlResolverOptions {
  fetch?: FetchLike;
  /**
   * Allows non-HTTPS or private/internal LNURL endpoints. Intended only for
   * local development or caller-provided URL allowlisting.
   */
  allowUnsafeUrls?: boolean;
}

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

function parseLnurlUrl(url: string, allowUnsafeUrls = false): URL {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new LniError('InvalidInput', 'Invalid LNURL URL.');
  }

  if (allowUnsafeUrls) {
    return parsed;
  }

  if (parsed.protocol !== 'https:') {
    throw new LniError('InvalidInput', 'LNURL endpoints must use HTTPS.');
  }

  if (parsed.username || parsed.password) {
    throw new LniError('InvalidInput', 'LNURL endpoints must not include credentials.');
  }

  if (isPrivateOrLocalHostname(parsed.hostname)) {
    throw new LniError('InvalidInput', 'LNURL endpoints must use a public hostname.');
  }

  return parsed;
}

function isPrivateOrLocalHostname(hostname: string): boolean {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, '').replace(/\.$/, '');
  if (!host) {
    return true;
  }

  if (
    host === 'localhost' ||
    host.endsWith('.localhost') ||
    host.endsWith('.local') ||
    host.endsWith('.internal')
  ) {
    return true;
  }

  if (host === '169.254.169.254') {
    return true;
  }

  if (isPrivateIpv4(host)) {
    return true;
  }

  return isPrivateIpv6(host);
}

function isPrivateIpv4(host: string): boolean {
  const parts = host.split('.');
  if (parts.length !== 4) {
    return false;
  }

  const octets = parts.map((part) => {
    if (!/^\d+$/.test(part)) {
      return Number.NaN;
    }
    return Number(part);
  });
  if (octets.some((octet) => !Number.isInteger(octet) || octet < 0 || octet > 255)) {
    return false;
  }

  const a = octets[0]!;
  const b = octets[1]!;
  return (
    a === 0 ||
    a === 10 ||
    a === 127 ||
    (a === 100 && b >= 64 && b <= 127) ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    (a === 198 && (b === 18 || b === 19))
  );
}

function isPrivateIpv6(host: string): boolean {
  const normalized = host.toLowerCase();
  if (!normalized.includes(':')) {
    return false;
  }

  if (normalized === '::1' || normalized === '::') {
    return true;
  }

  const firstSegment = normalized.split(':')[0] ?? '';
  const first = Number.parseInt(firstSegment, 16);
  if (Number.isInteger(first)) {
    if ((first & 0xfe00) === 0xfc00) {
      return true;
    }
    if ((first & 0xffc0) === 0xfe80) {
      return true;
    }
  }

  const mappedIpv4 = normalized.match(/(?:^|:)ffff:(\d+\.\d+\.\d+\.\d+)$/)?.[1];
  return mappedIpv4 ? isPrivateIpv4(mappedIpv4) : false;
}

async function fetchLnurlPay(
  url: string,
  fetchFn: FetchLike,
  options: Pick<LnurlResolverOptions, 'allowUnsafeUrls'> = {},
): Promise<LnurlPayResponse> {
  const parsedUrl = parseLnurlUrl(url, options.allowUnsafeUrls);
  const payload = await requestJson<LnurlPayResponse | LnurlErrorResponse>(fetchFn, parsedUrl.toString(), {
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

async function requestInvoice(
  callbackUrl: string,
  amountMsats: number,
  fetchFn: FetchLike,
  options: Pick<LnurlResolverOptions, 'allowUnsafeUrls'> = {},
): Promise<string> {
  const callback = parseLnurlUrl(callbackUrl, options.allowUnsafeUrls);
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

  validateInvoiceAmount(invoiceResponse.pr, amountMsats);

  return invoiceResponse.pr;
}

function invoiceAmountMsats(invoice: string): number | null {
  const decoded = decodeBolt11(invoice);
  const amount = decoded.sections.find((section) => section.name === 'amount');
  if (!amount || amount.name !== 'amount') {
    return null;
  }

  const parsed = Number(amount.value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw new LniError('InvalidInput', 'LNURL invoice amount is invalid.');
  }

  return parsed;
}

function validateInvoiceAmount(invoice: string, expectedAmountMsats: number): void {
  if (!Number.isSafeInteger(expectedAmountMsats) || expectedAmountMsats < 0) {
    throw new LniError('InvalidInput', 'LNURL invoice amount must be a non-negative safe integer.');
  }

  let actualAmountMsats: number | null;
  try {
    actualAmountMsats = invoiceAmountMsats(invoice);
  } catch (error) {
    if (error instanceof LniError) {
      throw error;
    }
    throw new LniError('InvalidInput', `Invalid LNURL invoice: ${(error as Error)?.message ?? 'unknown error'}`);
  }

  if (actualAmountMsats === null) {
    throw new LniError('InvalidInput', 'LNURL invoice is missing an amount.');
  }

  if (actualAmountMsats !== expectedAmountMsats) {
    throw new LniError(
      'InvalidInput',
      `LNURL invoice amount ${actualAmountMsats} msats does not match requested amount ${expectedAmountMsats} msats.`,
    );
  }
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

async function resolveViaLnurlPay(
  url: string,
  amountMsats: number,
  fetchFn: FetchLike,
  options: Pick<LnurlResolverOptions, 'allowUnsafeUrls'> = {},
): Promise<string> {
  const lnurlPay = await fetchLnurlPay(url, fetchFn, options);
  assertAmountRange(amountMsats, lnurlPay.minSendable, lnurlPay.maxSendable);
  return requestInvoice(lnurlPay.callback, amountMsats, fetchFn, options);
}

export async function resolveToBolt11(
  destination: string,
  amountMsats?: number,
  options: LnurlResolverOptions = {},
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
    return resolveViaLnurlPay(lightningAddressToUrl(user, domain), amountMsats, fetchFn, options);
  }

  const lnurl = decodeLnurl(destination.trim());
  return resolveViaLnurlPay(lnurl, amountMsats, fetchFn, options);
}

export async function getPaymentInfo(
  destination: string,
  amountMsats?: number,
  options: LnurlResolverOptions = {},
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
    const lnurlPay = await fetchLnurlPay(lightningAddressToUrl(user, domain), fetchFn, options);
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
  const lnurlPay = await fetchLnurlPay(lnurl, fetchFn, options);
  return {
    destinationType,
    destination,
    amountMsats,
    minSendableMsats: lnurlPay.minSendable,
    maxSendableMsats: lnurlPay.maxSendable,
    description: lnurlPay.metadata,
  };
}
