import type { InvoiceEventCallback, OnInvoiceEventParams, Transaction } from '../types.js';

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

export interface PollInvoiceEventsArgs {
  params: OnInvoiceEventParams;
  lookup: () => Promise<Transaction>;
  callback: InvoiceEventCallback;
}

export async function pollInvoiceEvents(args: PollInvoiceEventsArgs): Promise<void> {
  const delayMs = Math.max(args.params.pollingDelaySec, 1) * 1000;
  const maxDurationMs = Math.max(args.params.maxPollingSec, 1) * 1000;
  const startedAt = Date.now();

  const elapsed = () => Date.now() - startedAt;

  while (elapsed() < maxDurationMs) {
    const iterationStart = Date.now();

    try {
      const tx = await args.lookup();
      if (tx.settledAt > 0) {
        args.callback('success', tx);
        return;
      }
      args.callback('pending', tx);
    } catch (error) {
      if (typeof console !== 'undefined' && typeof console.debug === 'function') {
        console.debug('[lni] pollInvoiceEvents lookup failed (will retry)', error);
      }
      args.callback('pending');
    }

    // Sleep for the remaining delay time (accounting for how long the lookup took)
    const lookupDuration = Date.now() - iterationStart;
    const remainingSleep = Math.max(0, delayMs - lookupDuration);
    if (remainingSleep > 0 && elapsed() + remainingSleep < maxDurationMs) {
      await sleep(remainingSleep);
    } else if (elapsed() < maxDurationMs) {
      // Still have time but not enough for a full delay — do one more iteration
      continue;
    }
  }

  args.callback('failure');
}
