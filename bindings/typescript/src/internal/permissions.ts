import { decodeBase64 } from './encoding.js';
import type { Permissions } from '../types.js';

export const PERMISSION_GET_INFO = 'getInfo';
export const PERMISSION_CREATE_INVOICE = 'createInvoice';
export const PERMISSION_PAY_INVOICE = 'payInvoice';
export const PERMISSION_CREATE_OFFER = 'createOffer';
export const PERMISSION_GET_OFFER = 'getOffer';
export const PERMISSION_LIST_OFFERS = 'listOffers';
export const PERMISSION_PAY_OFFER = 'payOffer';
export const PERMISSION_LOOKUP_INVOICE = 'lookupInvoice';
export const PERMISSION_LIST_TRANSACTIONS = 'listTransactions';
export const PERMISSION_DECODE = 'decode';
export const PERMISSION_ON_INVOICE_EVENTS = 'onInvoiceEvents';

export const BOLT11_NODE_PERMISSIONS = [
  PERMISSION_GET_INFO,
  PERMISSION_CREATE_INVOICE,
  PERMISSION_PAY_INVOICE,
  PERMISSION_LOOKUP_INVOICE,
  PERMISSION_LIST_TRANSACTIONS,
  PERMISSION_DECODE,
  PERMISSION_ON_INVOICE_EVENTS,
] as const;

export const BOLT12_NODE_PERMISSIONS = [
  PERMISSION_CREATE_OFFER,
  PERMISSION_GET_OFFER,
  PERMISSION_LIST_OFFERS,
  PERMISSION_PAY_OFFER,
] as const;

export const FULL_NODE_PERMISSIONS = [
  ...BOLT11_NODE_PERMISSIONS,
  ...BOLT12_NODE_PERMISSIONS,
] as const;

export const NWC_METHOD_PERMISSIONS = [
  'get_info',
  'get_balance',
  'make_invoice',
  'pay_invoice',
  'lookup_invoice',
  'list_transactions',
] as const;

export const CLN_METHOD_PERMISSIONS = [
  'getinfo',
  'listfunds',
  'invoice',
  'pay',
  'offer',
  'fetchinvoice',
  'listoffers',
  'listinvoices',
  'decode',
] as const;

const STRIKE_SCOPE_PERMISSIONS: Array<{
  permission: string;
  scopes: string[];
}> = [
  {
    permission: PERMISSION_GET_INFO,
    scopes: ['partner.balances.read'],
  },
  {
    permission: PERMISSION_CREATE_INVOICE,
    scopes: ['partner.receive-request.create'],
  },
  {
    permission: PERMISSION_PAY_INVOICE,
    scopes: ['partner.payment-quote.lightning.create', 'partner.payment-quote.execute'],
  },
  {
    permission: PERMISSION_LOOKUP_INVOICE,
    scopes: ['partner.receive-request.read'],
  },
  {
    permission: PERMISSION_LIST_TRANSACTIONS,
    scopes: ['partner.receive-request.read', 'partner.payment.read'],
  },
  {
    permission: PERMISSION_ON_INVOICE_EVENTS,
    scopes: ['partner.receive-request.read'],
  },
];

const BLINK_SCOPE_PERMISSIONS: Array<{
  permission: string;
  scopes: string[];
}> = [
  {
    permission: PERMISSION_GET_INFO,
    scopes: ['read'],
  },
  {
    permission: PERMISSION_CREATE_INVOICE,
    scopes: ['receive'],
  },
  {
    permission: PERMISSION_PAY_INVOICE,
    scopes: ['write'],
  },
  {
    permission: PERMISSION_LOOKUP_INVOICE,
    scopes: ['read'],
  },
  {
    permission: PERMISSION_LIST_TRANSACTIONS,
    scopes: ['read'],
  },
  {
    permission: PERMISSION_ON_INVOICE_EVENTS,
    scopes: ['read'],
  },
];

export function emptyPermissions(): Permissions {
  return {
    getInfo: false,
    createInvoice: false,
    payInvoice: false,
    createOffer: false,
    getOffer: false,
    listOffers: false,
    payOffer: false,
    lookupInvoice: false,
    listTransactions: false,
    decode: false,
    onInvoiceEvents: false,
  };
}

export function isEmptyPermissions(permissions: Permissions): boolean {
  return Object.values(permissions).every((value) => !value);
}

export function normalizePermissionValues(values: Iterable<string | null | undefined>): string[] {
  const seen = new Set<string>();

  for (const value of values) {
    const normalized = value?.trim();
    if (normalized) {
      seen.add(normalized);
    }
  }

  return [...seen].sort((a, b) => a.localeCompare(b));
}

export function normalizePermissions(values: Iterable<string | null | undefined>): Permissions {
  return permissionsFromValues(compactPermissions(values));
}

export function normalizeNwcPermissions(values: Iterable<string | null | undefined>): Permissions {
  const permissions = compactPermissions(values);
  const has = (permission: string) => hasPermission(permissions, permission);

  return {
    ...emptyPermissions(),
    getInfo: has('get_balance'),
    createInvoice: has('make_invoice'),
    payInvoice: has('pay_invoice'),
    lookupInvoice: has('lookup_invoice'),
    listTransactions: has('list_transactions'),
    onInvoiceEvents: has('lookup_invoice'),
  };
}

export function normalizeClnPermissions(values: Iterable<string | null | undefined>): Permissions {
  const permissions = compactPermissions(values);
  const has = (permission: string) => hasPermission(permissions, permission);

  return {
    getInfo: has('getinfo') && has('listfunds'),
    createInvoice: has('invoice'),
    payInvoice: has('pay'),
    createOffer: has('offer'),
    getOffer: has('listoffers'),
    listOffers: has('listoffers'),
    payOffer: has('fetchinvoice') && has('pay'),
    lookupInvoice: has('listinvoices'),
    listTransactions: has('listinvoices'),
    decode: has('decode'),
    onInvoiceEvents: has('listinvoices'),
  };
}

export function normalizeLndPermissions(values: Iterable<string | null | undefined>): Permissions {
  const permissions = compactPermissions(values);
  const has = (permission: string) => hasPermission(permissions, permission);

  return {
    ...emptyPermissions(),
    getInfo:
      (has('/lnrpc.Lightning/GetInfo') && has('/lnrpc.Lightning/ChannelBalance')) ||
      (has('info:read') && has('offchain:read')),
    createInvoice: has('/lnrpc.Lightning/AddInvoice') || has('invoices:write'),
    payInvoice:
      has('/lnrpc.Lightning/SendPaymentSync') ||
      has('/routerrpc.Router/SendPaymentV2') ||
      has('offchain:write'),
    lookupInvoice: has('/lnrpc.Lightning/LookupInvoice') || has('invoices:read'),
    listTransactions:
      has('/lnrpc.Lightning/ListInvoices') ||
      has('/lnrpc.Lightning/ListPayments') ||
      has('invoices:read') ||
      has('offchain:read'),
    decode: has('/lnrpc.Lightning/DecodePayReq') || has('offchain:read'),
    onInvoiceEvents: has('/lnrpc.Lightning/SubscribeInvoices') || has('invoices:read'),
  };
}

export function parseClnRunePermissions(rune: string): Permissions {
  const decoded = decodeBase64Url(rune);
  const text = new TextDecoder().decode(decoded);
  const matches = text.match(/method(?:=|\^)[A-Za-z0-9_.:-]+/g) ?? [];

  if (!matches.length) {
    return normalizeClnPermissions(CLN_METHOD_PERMISSIONS);
  }

  const expanded: string[] = [];
  for (const permission of matches) {
    if (permission.startsWith('method=')) {
      expanded.push(permission.slice('method='.length));
      continue;
    }

    const prefix = permission.slice('method^'.length);
    const matchingMethods = CLN_METHOD_PERMISSIONS.filter((method) => method.startsWith(prefix));
    expanded.push(...(matchingMethods.length ? matchingMethods : [`${prefix}*`]));
  }

  return normalizeClnPermissions(expanded);
}

export function parseLndMacaroonPermissions(bytes: Uint8Array): Permissions {
  const text = new TextDecoder().decode(bytes);
  const matches = text.match(/[a-z][a-z0-9_-]*:(?:read|write|generate)/g) ?? [];
  return normalizeLndPermissions(matches);
}

export function getStrikeOauthPermissions(accessToken: string): Permissions | null {
  const payload = decodeJwtPayload(accessToken);
  if (!payload) {
    return null;
  }

  const scopes = new Set(readScopeValues(payload));
  const permissions = [PERMISSION_DECODE];

  for (const candidate of STRIKE_SCOPE_PERMISSIONS) {
    if (candidate.scopes.every((scope) => scopes.has(scope))) {
      permissions.push(candidate.permission);
    }
  }

  return normalizePermissions(permissions);
}

export function getGaloyTokenPermissions(token: string): Permissions | null {
  const payload = decodeJwtPayload(token);
  if (!payload) {
    return null;
  }

  const scopes = new Set(readScopeValues(payload).map((scope) => scope.toLowerCase()));
  const permissions = [PERMISSION_DECODE];

  for (const candidate of BLINK_SCOPE_PERMISSIONS) {
    if (candidate.scopes.every((scope) => scopes.has(scope))) {
      permissions.push(candidate.permission);
    }
  }

  return normalizePermissions(permissions);
}

/** @deprecated Use `getGaloyTokenPermissions`. */
export const getBlinkTokenPermissions = getGaloyTokenPermissions;

function permissionsFromValues(values: string[]): Permissions {
  const has = (permission: string) => hasPermission(values, permission);

  return {
    getInfo:
      has(PERMISSION_GET_INFO) ||
      has('get_info') ||
      has('getinfo') ||
      has('get_balance') ||
      has('/lnrpc.Lightning/GetInfo') ||
      has('/lnrpc.Lightning/ChannelBalance') ||
      has('info:read'),
    createInvoice:
      has(PERMISSION_CREATE_INVOICE) ||
      has('create_invoice') ||
      has('make_invoice') ||
      has('invoice') ||
      has('/lnrpc.Lightning/AddInvoice') ||
      has('invoices:write'),
    payInvoice:
      has(PERMISSION_PAY_INVOICE) ||
      has('pay_invoice') ||
      has('pay') ||
      has('/lnrpc.Lightning/SendPaymentSync') ||
      has('/routerrpc.Router/SendPaymentV2') ||
      has('offchain:write'),
    createOffer:
      has(PERMISSION_CREATE_OFFER) || has('create_offer') || has('offer') || has('offers:write'),
    getOffer:
      has(PERMISSION_GET_OFFER) || has('get_offer') || has('listoffers') || has('offers:read'),
    listOffers:
      has(PERMISSION_LIST_OFFERS) || has('list_offers') || has('listoffers') || has('offers:read'),
    payOffer:
      has(PERMISSION_PAY_OFFER) ||
      has('pay_offer') ||
      (has('fetchinvoice') && (has('pay') || has(PERMISSION_PAY_INVOICE))),
    lookupInvoice:
      has(PERMISSION_LOOKUP_INVOICE) ||
      has('lookup_invoice') ||
      has('lookup-invoice') ||
      has('listinvoices') ||
      has('/lnrpc.Lightning/LookupInvoice') ||
      has('invoices:read'),
    listTransactions:
      has(PERMISSION_LIST_TRANSACTIONS) ||
      has('list_transactions') ||
      has('listinvoices') ||
      has('/lnrpc.Lightning/ListInvoices') ||
      has('/lnrpc.Lightning/ListPayments') ||
      has('invoices:read') ||
      has('offchain:read'),
    decode: has(PERMISSION_DECODE) || has('/lnrpc.Lightning/DecodePayReq') || has('address:read'),
    onInvoiceEvents:
      has(PERMISSION_ON_INVOICE_EVENTS) ||
      has('on_invoice_events') ||
      has('lookup_invoice') ||
      has('listinvoices') ||
      has('/lnrpc.Lightning/SubscribeInvoices') ||
      has('invoices:read'),
  };
}

function compactPermissions(values: Iterable<string | null | undefined>): string[] {
  return [...values].flatMap((value) => {
    const normalized = value?.trim();
    return normalized ? [normalized] : [];
  });
}

function hasPermission(values: string[], permission: string): boolean {
  return values.some((value) => value.toLowerCase() === permission.toLowerCase());
}

function decodeJwtPayload(accessToken: string): Record<string, unknown> | null {
  const parts = accessToken.split('.');
  const payload = parts[1];
  if (parts.length < 3 || !payload) {
    return null;
  }

  try {
    return JSON.parse(new TextDecoder().decode(decodeBase64Url(payload))) as Record<
      string,
      unknown
    >;
  } catch {
    return null;
  }
}

function readScopeValues(payload: Record<string, unknown>): string[] {
  const values = [payload.scope, payload.scp, payload.scopes];
  const scopes: string[] = [];

  for (const value of values) {
    if (typeof value === 'string') {
      scopes.push(...value.split(/\s+/));
      continue;
    }

    if (Array.isArray(value)) {
      scopes.push(...value.filter((item): item is string => typeof item === 'string'));
    }
  }

  return normalizePermissionValues(scopes);
}

function decodeBase64Url(input: string): Uint8Array {
  const normalized = input.trim().replace(/-/g, '+').replace(/_/g, '/');
  const padding = (4 - (normalized.length % 4 || 4)) % 4;
  return decodeBase64(normalized + '='.repeat(padding));
}
