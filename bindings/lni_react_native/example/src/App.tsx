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
  TEST_RECEIVER_OFFER,
} from '@env';
// import RNFS from 'react-native-fs';

export default function App() {
  const [offer, setOffer] = useState<string>('');
  const [pubKey, setPubKey] = useState<string>('');
  const [invoice, setInvoice] = useState<string | number>('');
  const [txns, setTxns] = useState<any>('');
  const [payment, setPayment] = useState<any>('');

  const main = async () => {
    phoenixd();
    cln();
  };

  async function phoenixd() {
    try {
      const node = new PhoenixdNode({
        url: PHOENIXD_URL,
        password: PHOENIXD_PASSWORD,
      });

      //const info = await node.getInfo();
      // setPubKey(info.pubkey);

      const offerResp = await node.makeInvoice({
        invoiceType: InvoiceType.Bolt12,
        amountMsats: BigInt(1000),
        description: 'Test invoice',
        descriptionHash: undefined,
        expiry: undefined,
      });
      setOffer(offerResp.invoice);

      const lookupInvoice = await node.lookupInvoice(
        PHOENIXD_TEST_PAYMENT_HASH
      );
      setInvoice(Number(lookupInvoice.amountMsats));

      let txnParams: ListTransactionsParams = {
        from: BigInt(0),
        limit: BigInt(10),
        paymentHash: undefined, // TODO figure out how to exclude this instead of passing in undefined
      };
      const txns = await node.listTransactions(txnParams);
      setTxns(JSON.stringify(txns[0], bigIntReplacer));

      const paymentResp = await node.payOffer(
        TEST_RECEIVER_OFFER,
        BigInt(3000),
        'payment from react-native'
      );
      console.log('Pay offer resposne', paymentResp);
      setPayment(JSON.stringify(paymentResp, bigIntReplacer));
    } catch (e) {
      console.error('Error', e);
    }
  }
  function cln() {}

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
      <Text>DB: {payment}</Text>
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
