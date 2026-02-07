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

  while (Date.now() - startedAt <= maxDurationMs) {
    try {
      const tx = await args.lookup();
      if (tx.settledAt > 0) {
        args.callback('success', tx);
        return;
      }
      args.callback('pending', tx);
    } catch {
      args.callback('failure');
    }

    await sleep(delayMs);
  }

  args.callback('failure');
}
