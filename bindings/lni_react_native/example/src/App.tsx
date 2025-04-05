import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { LndNodeUniffi, LndConfig } from 'lni_react_native';

export default function App() {
  const [result, setResult] = useState<string>('Loading...');

  useEffect(() => {
    const runRustCode = async () => {
      try {
        setResult('Done');
        // Initialize the Rust library
        const node = new LndNodeUniffi(LndConfig.create({
          url: '',
          macaroon: '',
          socks5Proxy: '',
        }));
        const info = await node.getInfo();
        setResult(
          JSON.stringify(info, (_, value) =>
            typeof value === 'bigint' ? value.toString() : value
          )
        );
        // console.log('Rust library initialized');
      } catch (error) {
        console.error('Error initializing Rust library:', error);
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
