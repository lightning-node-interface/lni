import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { Fetcher, PhoenixdNode } from '../../src';

export default function App() {
  const fetcher = new Fetcher('http://woot.com');
  const [ip, setIp] = useState<string>('');
  const [offer, setOffer] = useState<string>('');
  const [config, setConfig] = useState<string>('');

  const main = async () => {
    const conf = fetcher.getConfig();
    setConfig(conf);
    console.log('Config', conf);

    try {
      const node = new PhoenixdNode({
        url: 'http://localhost:9740',
        password: '',
      });

      const offer = await node.getOffer();
      setOffer(offer);
    } catch (e) {
      console.error('Error', e);
    }

    try {
      const ipRes = await fetcher.getIpAddress();
      setIp(ipRes.origin ?? 'none');
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
      <Text>Config: {config}</Text>
      <Text>IP: {ip}</Text>
      <Text>Offer: {offer}</Text>
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
