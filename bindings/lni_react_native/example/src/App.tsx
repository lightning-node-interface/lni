import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import {
  LndNode,
  LndConfig,
  PhoenixdNode,
  PhoenixdConfig,
  type OnInvoiceEventCallback,
  Transaction,
} from 'lni_react_native';
import { LND_URL, LND_MACAROON } from '@env';

// class InvoiceCallback implements OnInvoiceEventCallback {
//   success(transaction: Transaction | undefined): void {
//     console.log('Received pending invoice event:', transaction);
//   }
//   pending(transaction: Transaction | undefined): void {
//     console.log('Received success event:', transaction);
//   }
//   failure(transaction: Transaction | undefined): void {
//     console.log('Received failure event:', transaction);
//   }
// }

export default function App() {
  const [result, setResult] = useState<string>('Loading...');

  useEffect(() => {
    const runRustCode = async () => {
      try {
        const node = new LndNode(
          LndConfig.create({
            url: '',
            macaroon:
              '',
            socks5Proxy: undefined, // 'socks5h://127.0.0.1:9050',
          })
        );

        await node.onInvoiceEvents(
          {
            paymentHash: ''
            pollingDelaySec: BigInt(3),
            maxPollingSec: BigInt(60),
          },
          {
            success(transaction: Transaction | undefined): void {
              console.log('Received pending invoice event:', transaction);
              setResult('Success');
            },
            pending(transaction: Transaction | undefined): void {
              console.log('Received success event:', transaction);
            },
            failure(transaction: Transaction | undefined): void {
              console.log('Received failure event:', transaction);
            },
          }
        );

        // const info = await node.listTransactions({
        //   from: BigInt(0),
        //   limit: BigInt(10),
        //   paymentHash: undefined,
        // });
        // setResult(
        //   JSON.stringify(info, (_, value) =>
        //     typeof value === 'bigint' ? value.toString() : value
        //   )
        // );
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
