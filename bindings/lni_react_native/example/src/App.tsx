import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { LndNodeUniffi, LndConfig } from 'lni_react_native';
import { LND_URL, LND_MACAROON } from '@env';

export default function App() {
  const [result, setResult] = useState<string>('Loading...');

  useEffect(() => {
    const runRustCode = async () => {
      try {
        const node = new LndNodeUniffi(LndConfig.create({
          url: '', //LND_URL,
          macaroon: '', // LND_MACAROON,
          socks5Proxy: "socks5h://127.0.0.1:9050",
        }));
        const info = await node.getInfo();
        setResult(
          JSON.stringify(info, (_, value) =>
            typeof value === 'bigint' ? value.toString() : value
          )
        );
      } catch (error) {
        console.error('Error initializing LNI Remote library:', error);
      }
    };
    runRustCode();
  }, []);

  return (
    <View style={styles.container}>
      <Text>Result: {result}</Text>
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
