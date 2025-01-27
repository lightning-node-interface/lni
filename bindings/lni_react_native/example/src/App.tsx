import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { Fetcher, PhoenixdNode, type Channel } from '../../src';
import { PHOENIXD_URL, PHOENIXD_PASSWORD } from '@env';

export default function App() {
  const fetcher = new Fetcher('http://woot.com');
  const [ip, setIp] = useState<string>('');
  const [offer, setOffer] = useState<string>('');
  const [pubKey, setPubKey] = useState<string>('');
  const [config, setConfig] = useState<string>('');

  const main = async () => {
    const conf = fetcher.getConfig();
    setConfig(conf);
    console.log('Config', conf);

    const c: Channel = {
      localBalance: BigInt(100),
      localSpendableBalance: BigInt(100),
      remoteBalance: BigInt(100),
      id: 'string',
      remotePubkey: 'string',
      fundingTxId: 'string',
      fundingTxVout: BigInt(100),
      active: true,
      public_: true,
      internalChannel: 'string',
      confirmations: BigInt(100),
      confirmationsRequired: BigInt(100),
      forwardingFeeBaseMsat: BigInt(100),
      unspendablePunishmentReserve: BigInt(100),
      counterpartyUnspendablePunishmentReserve: BigInt(100),
      error: 'string',
      isOutbound: true,
    };

    console.log('Channel', c);

    try {
      const node = new PhoenixdNode({
        url: PHOENIXD_URL,
        password: PHOENIXD_PASSWORD,
      });

      const info = await node.getInfo();
      setPubKey(info.pubkey);
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
      <Text>Node PubKey: {pubKey}</Text>
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
