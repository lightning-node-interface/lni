import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { LndNode } from 'lni_react_native';

export default function App() {
  const [result, setResult] = useState<string>('Loading...');

  useEffect(() => {
    const runRustCode = async () => {
      try {
        setResult('Done');
        // Initialize the Rust library
        const node = new LndNode(
          '', //url: string,
          '', //macaroon: string,
          'socks5h://127.0.0.1:9050', //socks5Proxy: string | undefined,
          true, //acceptInvalidCerts: boolean | undefined,
          BigInt(60), //httpTimeout: /*i64*/ bigint | undefined,
        );
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
