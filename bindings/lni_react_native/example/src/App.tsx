import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { PhoenixdNode, InvoiceType } from '../../src';
import { PHOENIXD_URL, PHOENIXD_PASSWORD } from '@env';

export default function App() {
  const [offer, setOffer] = useState<string>('');
  const [pubKey, setPubKey] = useState<string>('');
  const [config, setConfig] = useState<string>('');

  const main = async () => {
    console.log('Channel', c);

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
      });
      setOffer(offerResp.invoice);
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
