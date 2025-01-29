import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import {
  PhoenixdNode,
  InvoiceType,
  type ListTransactionsParams,
} from '../../src';
import {
  PHOENIXD_URL,
  PHOENIXD_PASSWORD,
  PHOENIXD_TEST_PAYMENT_HASH,
} from '@env';

export default function App() {
  const [offer, setOffer] = useState<string>('');
  const [pubKey, setPubKey] = useState<string>('');
  const [invoice, setInvoice] = useState<string | number>('');
  const [txns, setTxns] = useState<any>('');

  const main = async () => {
    try {
      const node = new PhoenixdNode({
        url: PHOENIXD_URL,
        password: PHOENIXD_PASSWORD,
      });

      const info = await node.getInfo();
      setPubKey(info.pubkey);

      const offerResp = await node.makeInvoice({
        invoiceType: InvoiceType.Bolt12,
        amount: BigInt(1000),
        description: 'Test invoice',
        descriptionHash: undefined,
        expiry: undefined,
      });
      setOffer(offerResp.invoice);

      const lookupInvoice = await node.lookupInvoice(
        PHOENIXD_TEST_PAYMENT_HASH
      );
      setInvoice(Number(lookupInvoice.amount));

      let txnParams: ListTransactionsParams = {
        from: BigInt(0),
        offset: BigInt(0),
        limit: BigInt(10),
        invoiceType: 'all',
        unpaid: false,
        until: BigInt(0),
      };
      const txns = await node.listTransactions(txnParams);
      setTxns(JSON.stringify(txns[0], bigIntReplacer));
    } catch (e) {
      console.error('Error', e);
    }
  };

  useEffect(() => {
    setTimeout(() => {
      console.log('Starting main');
      main();
    }, 1000);
  }, []);

  return (
    <View style={styles.container}>
      <Text />
      <Text>Node PubKey: {pubKey}</Text>
      <Text />
      <Text>Offer: {offer}</Text>
      <Text />
      <Text>Lookup Invoice Amt: {invoice}</Text>
      <Text />
      <Text>First Txn: {txns}</Text>
      <Text />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
});

function bigIntReplacer(key: any, value: any) {
  if (typeof value === 'bigint') {
    return value.toString();
  }
  return value;
}