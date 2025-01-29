import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { PhoenixdNode, InvoiceType } from '../../src';
import {
  PHOENIXD_URL,
  PHOENIXD_PASSWORD,
  PHOENIXD_TEST_PAYMENT_HASH,
} from '@env';

export default function App() {
  const [offer, setOffer] = useState<string>('');
  const [pubKey, setPubKey] = useState<string>('');
  const [invoice, setInvoice] = useState<string | number>('');

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
