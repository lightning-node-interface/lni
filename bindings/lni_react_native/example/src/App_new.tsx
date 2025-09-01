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
  const [result, setResult] = useState<string>('Ready to test Promise-based NWC polling...');
  const [isPolling, setIsPolling] = useState(false);
  const [pollCount, setPollCount] = useState(0);
  const abortControllerRef = useRef<AbortController | null>(null);

  const testNwcCancellation = async () => {
    try {
      setResult('Starting Promise-based NWC polling...');
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

      // Create AbortController for proper cancellation
      const abortController = new AbortController();
      abortControllerRef.current = abortController;

      console.log('🔧 Starting Promise-based NWC polling');

      // Manual polling loop with proper async/await and cancellation
      const pollAsync = async () => {
        const startTime = Date.now();
        let currentPollCount = 0;

        while (!abortController.signal.aborted) {
          currentPollCount++;
          setPollCount(currentPollCount);
          setResult(`🔄 Poll #${currentPollCount}: Checking invoice status...`);

          console.log(`🔍 Poll #${currentPollCount}: Looking up invoice`);

          try {
            // For now, use the synchronous version until we rebuild with async version
            const { nwcLookupInvoice } = require('lni_react_native');
            
            // Call the Rust function (this will be sync until we rebuild)
            const transaction = await new Promise((resolve, reject) => {
              try {
                const result = nwcLookupInvoice(config, params.paymentHash, params.search);
                resolve(result);
              } catch (error) {
                reject(error);
              }
            });

            console.log(`✅ Poll #${currentPollCount}: lookup succeeded`, transaction);

            if (transaction && (transaction as any).settledAt > 0) {
              setResult(`✅ Poll #${currentPollCount}: Payment settled! Hash: ${(transaction as any).paymentHash}`);
              setIsPolling(false);
              abortControllerRef.current = null;
              return;
            } else {
              setResult(`⏳ Poll #${currentPollCount}: Invoice still pending...`);
            }

          } catch (error) {
            console.log(`❌ Poll #${currentPollCount}: lookup failed:`, error);
            setResult(`❌ Poll #${currentPollCount}: ${error}`);
            
            // Stop after 10 failed attempts
            if (currentPollCount >= 10) {
              setResult(`❌ Stopped after ${currentPollCount} failed attempts`);
              setIsPolling(false);
              abortControllerRef.current = null;
              return;
            }
          }

          // Check timeout
          const elapsed = Date.now() - startTime;
          if (elapsed > 30000) {
            setResult(`⏰ Timeout after ${currentPollCount} polls`);
            setIsPolling(false);
            abortControllerRef.current = null;
            return;
          }

          // Wait 3 seconds before next poll (checking for cancellation every 500ms)
          for (let i = 0; i < 6; i++) {
            if (abortController.signal.aborted) {
              setResult(`🛑 Cancelled after ${currentPollCount} polls`);
              setIsPolling(false);
              abortControllerRef.current = null;
              return;
            }
            await new Promise(resolve => setTimeout(resolve, 500));
          }
        }

        if (abortController.signal.aborted) {
          setResult(`🛑 Cancelled after ${currentPollCount} polls`);
          setIsPolling(false);
          abortControllerRef.current = null;
        }
      };

      // Start the polling
      pollAsync().catch((error) => {
        if (!abortController.signal.aborted) {
          setResult(`❌ Polling error: ${error}`);
          setIsPolling(false);
          abortControllerRef.current = null;
        }
      });

    } catch (error) {
      console.log('❌ Error starting NWC polling:', error);
      setResult(`❌ Error: ${error}`);
      setIsPolling(false);
      abortControllerRef.current = null;
    }
  };

  const cancelPolling = () => {
    console.log('🛑 Cancel button clicked...');
    if (abortControllerRef.current && !abortControllerRef.current.signal.aborted) {
      console.log('🛑 Aborting polling...');
      abortControllerRef.current.abort();
      setResult('🛑 Cancel requested!');
      
      // The polling loop will detect the abort and clean up
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
        setResult('✅ LNI library loaded successfully! Ready to test Promise-based NWC polling.');
      } catch (error) {
        console.error('Error initializing LNI library:', error);
        setResult(`⚠️ Library loaded, but LND connection failed (expected): ${error}`);
      }
    };
    runRustCode();
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Promise-based NWC Polling Test</Text>
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
          disabled={!isPolling}
          color="red"
        />
      </View>
      
      <View style={styles.statusContainer}>
        <Text style={styles.statusText}>
          Status: {isPolling ? `🔄 Polling... (${pollCount} attempts)` : '⏸️ Stopped'}
        </Text>
        <Text style={styles.helperText}>
          {isPolling 
            ? "🔥 UI should remain responsive! Try tapping Cancel." 
            : "Tap 'Start' to begin Promise-based polling with proper cancellation."
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
