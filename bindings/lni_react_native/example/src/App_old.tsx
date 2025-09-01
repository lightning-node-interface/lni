import { useEffect, useState, useRef } from 'react';
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
  const [isPolling, setIsPolling] = useState(false);
  const [pollCount, setPollCount] = useState(0);
  const abortControllerRef = useRef<AbortController | null>(null);

  const testNwcCancellation = async () => {
    try {
      setResult('Starting NWC invoice event polling with cancellation...');
      setIsPolling(true);
      setPollCount(0);

      const config = NwcConfig.create({
        nwcUri: '',
        socks5Proxy: '',
      });

      const params = OnInvoiceEventParams.create({
        paymentHash: "",
        search: undefined,
        pollingDelaySec: BigInt(3),
        maxPollingSec: BigInt(30),
      });

      const callback = {
        success(transaction: Transaction | undefined): void {
          console.log('✅ NWC Success event:', transaction);
          // Force state update on next tick
          setImmediate(() => {
            setPollCount(prev => prev + 1);
            setResult(`✅ Payment successful! Hash: ${transaction?.paymentHash}`);
            setIsPolling(false);
            setCancellation(null);
            if (pollTimer) {
              clearInterval(pollTimer);
              setPollTimer(null);
            }
          });
        },
        pending(transaction: Transaction | undefined): void {
          console.log('⏳ NWC Pending event:', transaction);
          // Force state update on next tick
          setImmediate(() => {
            setPollCount(prev => {
              const newCount = prev + 1;
              setResult(`⏳ Poll #${newCount}: Still pending - Hash: ${transaction?.paymentHash || 'N/A'}`);
              return newCount;
            });
          });
        },
        failure(transaction: Transaction | undefined): void {
          console.log('❌ NWC Failure event:', transaction);
          // Force state update on next tick
          setImmediate(() => {
            setPollCount(prev => {
              const newCount = prev + 1;
              const reason = cancellation?.isCancelled() ? 'Cancelled by user' : 'lookup_invoice failed';
              setResult(`❌ Poll #${newCount}: ${reason}! ${transaction?.paymentHash || 'No hash'}`);
              
              // Stop after 10 failed attempts
              if (newCount >= 10 && !cancellation?.isCancelled()) {
                setResult(`❌ Stopped after ${newCount} failed attempts. Check NWC config.`);
                setIsPolling(false);
                setCancellation(null);
              }
              
              return newCount;
            });
          });
        },
      };

      console.log('🔧 Starting NWC polling with config:', {
        paymentHash: params.paymentHash,
        pollingDelaySec: params.pollingDelaySec.toString(),
        maxPollingSec: params.maxPollingSec.toString(),
      });

      // Start the actual NWC polling
      const cancellationHandle = nwcOnInvoiceEventsWithCancellation(config, params, callback);
      setCancellation(cancellationHandle);
      
      console.log('� Cancellation handle created, polling should start now...');
      setResult('🔄 NWC polling started! Waiting for callbacks...');

    } catch (error) {
      console.log('❌ Error testing NWC cancellation:', error);
      setResult(`❌ Error: ${error}`);
      setIsPolling(false);
      setCancellation(null);
      if (pollTimer) {
        clearInterval(pollTimer);
        setPollTimer(null);
      }
    }
  };

  const cancelPolling = () => {
    console.log('🛑 Cancel button clicked...');
    if (cancellation) {
      console.log('🛑 Calling cancellation.cancel()...');
      cancellation.cancel();
      
      // Check if it was actually cancelled
      const isCancelled = cancellation.isCancelled();
      console.log('Cancellation status:', isCancelled); 
      
      setResult(`🛑 Cancel requested! Status: ${isCancelled ? 'Cancelled' : 'Still Active'}`);
      
      // Don't immediately clear state - let the callback handle cleanup
      if (isCancelled) {
        setTimeout(() => {
          setIsPolling(false);
          setCancellation(null);
          if (pollTimer) {
            clearInterval(pollTimer);
            setPollTimer(null);
          }
        }, 1000); // Give the polling loop time to detect cancellation
      }
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
          disabled={isPolling}
        />
        
        <Button
          title="Cancel Polling"
          onPress={cancelPolling}
          disabled={!isPolling || !cancellation}
          color="red"
        />
      </View>
      
      <View style={styles.statusContainer}>
        <Text style={styles.statusText}>
          Status: {isPolling ? `🔄 Polling... (${pollCount} attempts)` : '⏸️ Stopped'}
        </Text>
        {cancellation && (
          <Text style={styles.statusText}>
            Cancellation: {cancellation.isCancelled() ? '❌ Cancelled' : '✅ Active'}
          </Text>
        )}
        <Text style={styles.helperText}>
          {isPolling 
            ? "🔥 UI should remain responsive! Try tapping Cancel." 
            : "Tap 'Start' to begin polling, then test cancellation."
          }
        </Text>
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
  helperText: {
    fontSize: 12,
    fontStyle: 'italic',
    textAlign: 'center',
    marginTop: 10,
    color: '#666',
    paddingHorizontal: 20,
  },
});
