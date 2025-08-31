import { useEffect, useState } from 'react';
import { Text, View, StyleSheet, Button, Alert } from 'react-native';
import {
  LndNode,
  LndConfig,
  PhoenixdNode,
  PhoenixdConfig,
  type OnInvoiceEventCallback,
  Transaction,
  BlinkConfig,
  BlinkNode,
  NwcConfig,
  OnInvoiceEventParams,
  nwcOnInvoiceEventsWithCancellation,
  type InvoiceEventsCancellationInterface,
} from 'lni_react_native';
import { LND_URL, LND_MACAROON } from '@env';

export default function App() {
  const [result, setResult] = useState<string>('Ready to test NWC cancellation...');
  const [cancellation, setCancellation] = useState<InvoiceEventsCancellationInterface | null>(null);
  const [isPolling, setIsPolling] = useState(false);

  const testNwcCancellation = async () => {
    try {
      setResult('Starting NWC invoice event polling with cancellation...');
      setIsPolling(true);

      // Use a simpler test configuration
      const config = NwcConfig.create({
        nwcUri: 'nostr+walletconnect://test',
        socks5Proxy: '', // empty string instead of null
      });

      // Use a complete params object to avoid any parameter issues
      const params = OnInvoiceEventParams.create({
        paymentHash: undefined,
        search: undefined,
        pollingDelaySec: BigInt(5),
        maxPollingSec: BigInt(60),
      });

      const callback = {
        success(transaction: Transaction | undefined): void {
          console.log('✅ NWC Success event:', transaction);
          setResult(`✅ Success! Payment completed.`);
          setIsPolling(false);
        },
        pending(transaction: Transaction | undefined): void {
          console.log('⏳ NWC Pending event:', transaction);
          setResult(`⏳ Pending... Still waiting for payment.`);
        },
        failure(transaction: Transaction | undefined): void {
          console.log('❌ NWC Failure event:', transaction);
          setResult(`❌ Failed! Payment failed.`);
          setIsPolling(false);
        },
      };

      // Start cancellable invoice event monitoring
      const cancellationHandle = nwcOnInvoiceEventsWithCancellation(config, params, callback);
      setCancellation(cancellationHandle);

      setResult('🔄 Polling for invoice events... Use "Cancel" to stop or wait 30 seconds for timeout.');

    } catch (error) {
      console.error('❌ Error testing NWC cancellation:', error);
      setResult(`❌ Error: ${error}`);
      setIsPolling(false);
    }
  };

  const cancelPolling = () => {
    console.log('🛑 Click Cancelling invoice event polling...');
    if (cancellation) {
      console.log('🛑 Cancelling invoice event polling...');
      cancellation.cancel();
      
      // Check if it was actually cancelled
      const isCancelled = cancellation.isCancelled();
      console.log('Cancellation status:', isCancelled); 
      
      setResult(`🛑 Cancelled! Polling stopped. Was cancelled: ${isCancelled}`);
      setCancellation(null);
      setIsPolling(false);
    } else {
      Alert.alert('No Active Polling', 'There is no active polling to cancel.');
    }
  };

  useEffect(() => {
    const runRustCode = async () => {
      try {
        // Test basic functionality first
        const node = new LndNode(
          LndConfig.create({
            url: '',
            macaroon: '',
            socks5Proxy: '', // empty string instead of undefined
          })
        );

        // Don't try to connect to LND since we don't have valid credentials
        // Just test that the library loads correctly
        setResult('✅ LNI library loaded successfully! Ready to test NWC cancellation.');
      } catch (error) {
        console.error('Error initializing LNI library:', error);
        setResult(`⚠️ Library loaded, but LND connection failed (expected): ${error}`);
      }
    };
    runRustCode();
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.title}>NWC Cancellation Test</Text>
      <Text style={styles.result}>{result}</Text>
      
      <View style={styles.buttonContainer}>
        <Button
          title="Start NWC Polling"
          onPress={testNwcCancellation}
          //disabled={isPolling}
        />
        
        <Button
          title="Cancel Polling"
          onPress={cancelPolling}
          //disabled={!isPolling || !cancellation}
          color="red"
        />
      </View>
      
      <View style={styles.statusContainer}>
        <Text style={styles.statusText}>
          Status: {isPolling ? '🔄 Polling...' : '⏸️ Stopped'}
        </Text>
        {cancellation && (
          <Text style={styles.statusText}>
            Cancellable: {cancellation.isCancelled() ? '❌ Cancelled' : '✅ Active'}
          </Text>
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 20,
  },
  title: {
    fontSize: 20,
    fontWeight: 'bold',
    marginBottom: 20,
    textAlign: 'center',
  },
  result: {
    fontSize: 14,
    textAlign: 'center',
    marginVertical: 20,
    paddingHorizontal: 10,
  },
  buttonContainer: {
    flexDirection: 'row',
    gap: 10,
    marginVertical: 20,
  },
  statusContainer: {
    marginTop: 20,
    alignItems: 'center',
  },
  statusText: {
    fontSize: 16,
    marginVertical: 5,
  },
});
