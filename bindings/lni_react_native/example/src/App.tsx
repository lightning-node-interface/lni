import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { PhoenixdNode } from '../../src';

export default function App() {
  const fetcher = new Fetcher('http://woot.com');
  const [ip, setIp] = useState<string>('');

  const main = async () => {
    const config = fetcher.getConfig();
    setIp(config);
    console.log('Config', config);
  };

  useEffect(() => {
    main();
  }, []);

  return (
    <View style={styles.container}>
      <Text>Config: {ip}</Text>
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
