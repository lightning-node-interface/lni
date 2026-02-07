import { it } from 'vitest';
import 'websocket-polyfill';

export const timeout = 120_000;
type ConditionalIt = (name: string, fn: () => Promise<void> | void, timeout?: number) => void;
export const itIf = (condition: boolean): ConditionalIt => (condition ? it : it.skip);

export const hasEnv = (...keys: string[]): boolean =>
  keys.every((key) => Boolean(process.env[key]?.trim()));

export const nonEmpty = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
};

export function uniqueValues(values: Array<string | undefined>): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const value of values) {
    const normalized = nonEmpty(value);
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    result.push(normalized);
  }

  return result;
}

export function testInvoiceLabel(prefix: string): string {
  return `${prefix} ts integration ${Date.now()}`;
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function isKnownError(error: unknown, patterns: string[]): boolean {
  const message = errorMessage(error).toLowerCase();
  return patterns.some((pattern) => message.includes(pattern.toLowerCase()));
}

export async function runOrSkipKnownError(action: () => Promise<void>, patterns: string[]): Promise<void> {
  try {
    await action();
  } catch (error) {
    if (isKnownError(error, patterns)) {
      return;
    }
    throw error;
  }
}
