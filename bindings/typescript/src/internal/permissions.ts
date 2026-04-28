import { decodeBase64 } from './encoding.js';

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
    scopes: [
      'partner.payment-quote.lightning.create',
      'partner.payment-quote.execute',
    ],
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

export function normalizePermissions(values: Iterable<string | null | undefined>): string[] {
  const seen = new Set<string>();

  for (const value of values) {
    const normalized = value?.trim();
    if (normalized) {
      seen.add(normalized);
    }
  }

  return [...seen].sort((a, b) => a.localeCompare(b));
}

export function parseClnRunePermissions(rune: string): string[] {
  const decoded = decodeBase64Url(rune);
  const text = new TextDecoder().decode(decoded);
  const matches = text.match(/method(?:=|\^)[A-Za-z0-9_.:-]+/g) ?? [];

  if (!matches.length) {
    return normalizePermissions(CLN_METHOD_PERMISSIONS);
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

  return normalizePermissions(expanded);
}

export function parseLndMacaroonPermissions(bytes: Uint8Array): string[] {
  const text = new TextDecoder().decode(bytes);
  const matches = text.match(/[a-z][a-z0-9_-]*:(?:read|write|generate)/g) ?? [];
  return normalizePermissions(matches);
}

export function getStrikeOauthPermissions(accessToken: string): string[] | null {
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

export function getBlinkTokenPermissions(token: string): string[] | null {
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

function decodeJwtPayload(accessToken: string): Record<string, unknown> | null {
  const parts = accessToken.split('.');
  const payload = parts[1];
  if (parts.length < 3 || !payload) {
    return null;
  }

  try {
    return JSON.parse(new TextDecoder().decode(decodeBase64Url(payload))) as Record<string, unknown>;
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

  return normalizePermissions(scopes);
}

function decodeBase64Url(input: string): Uint8Array {
  const normalized = input.trim().replace(/-/g, '+').replace(/_/g, '/');
  const padding = (4 - (normalized.length % 4 || 4)) % 4;
  return decodeBase64(normalized + '='.repeat(padding));
}
