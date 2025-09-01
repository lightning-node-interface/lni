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
  nwcStartInvoicePolling,
  type InvoicePollingStateInterface,
} from 'lni_react_native';
import { LND_URL, LND_MACAROON, NWC_URI, NWC_TEST_PAYMENT_HASH } from '@env';

export default function App() {
  const [result, setResult] = useState<string>('Ready to test new InvoicePollingState approach...');
  const [isPolling, setIsPolling] = useState(false);
  const [pollCount, setPollCount] = useState(0);
  const pollingStateRef = useRef<InvoicePollingStateInterface | null>(null);

  // Helper function to safely serialize objects with BigInt values
  const safetStringify = (obj: any, indent = 2) => {
    return JSON.stringify(obj, (key, value) => {
      if (typeof value === 'bigint') {
        return value.toString();
      }
      return value;
    }, indent);
  };

  const testNwcPolling = async () => {
    try {
      // Validate environment variables
      if (!NWC_URI) {
        setResult('❌ Error: NWC_URI not found in environment variables');
        return;
      }
      if (!NWC_TEST_PAYMENT_HASH) {
        setResult('❌ Error: NWC_TEST_PAYMENT_HASH not found in environment variables');
        return;
      }

      setResult('Starting InvoicePollingState-based NWC polling...');
      setIsPolling(true);
      setPollCount(0);

      const config = NwcConfig.create({
        nwcUri: NWC_URI,
      });

      const params = OnInvoiceEventParams.create({
        paymentHash: NWC_TEST_PAYMENT_HASH,
        search: undefined,
        pollingDelaySec: BigInt(2),
        maxPollingSec: BigInt(15),
      });

      console.log('🔧 Starting InvoicePollingState-based NWC polling');
      console.log('🔧 Using NWC_URI from env:', NWC_URI.substring(0, 50) + '...');
      console.log('🔧 Using payment hash from env:', NWC_TEST_PAYMENT_HASH);

      // Start the polling using the new function
      console.log('📋 Config:', safetStringify(config));
      console.log('📋 Params:', safetStringify(params));
      const pollingState = nwcStartInvoicePolling(config, params);
      console.log('📋 PollingState created:', safetStringify(pollingState));
      pollingStateRef.current = pollingState;

      // Check initial state
      console.log(`📋 Initial poll count: ${pollingState.getPollCount()}`);
      console.log(`📋 Initial status: ${pollingState.getLastStatus()}`);
      console.log(`📋 Initial transaction: ${pollingState.getLastTransaction() ? 'present' : 'null'}`);
      console.log(`📋 Initial cancelled: ${pollingState.isCancelled()}`);

      // Give it a moment to start
      await new Promise(resolve => setTimeout(resolve, 100));

      // Monitor the polling state
      const monitorPolling = async () => {
        const startTime = Date.now();

        while (!pollingState.isCancelled()) {
          const currentCount = pollingState.getPollCount();
          const currentStatus = pollingState.getLastStatus();
          const lastTransaction = pollingState.getLastTransaction();

          setPollCount(currentCount);

          console.log(`📊 Poll #${currentCount}: Status: ${currentStatus}`);
          console.log(`📊 Poll #${currentCount}: Transaction: ${lastTransaction ? 'present' : 'null'}`);
          console.log(`📊 Poll #${currentCount}: Is cancelled: ${pollingState.isCancelled()}`);
          
          // Print detailed transaction info if present
          if (lastTransaction) {
            console.log(`📊 Poll #${currentCount}: Transaction details:`, safetStringify(lastTransaction));
          }

          if (currentStatus === 'success' && lastTransaction) {
            console.log(`✅ Poll #${currentCount}: SUCCESS - Payment settled!`);
            setResult(`✅ Poll #${currentCount}: Payment settled! Transaction: ${safetStringify(lastTransaction).substring(0, 100)}...`);
            setIsPolling(false);
            pollingStateRef.current = null;
            return;
          } else if (currentStatus === 'failure') {
            console.log(`❌ Poll #${currentCount}: FAILURE - Polling failed`);
            setResult(`❌ Poll #${currentCount}: Polling failed with status: ${currentStatus}`);
            //setIsPolling(false);
            //pollingStateRef.current = null;
          } else {
            console.log(`🔄 Poll #${currentCount}: CONTINUING - Status: ${currentStatus || 'pending'}`);
            setResult(`🔄 Poll #${currentCount}: Status: ${currentStatus || 'pending'} - ${lastTransaction ? 'Transaction found' : 'No transaction yet'}`);
          }

          // Check timeout
          const elapsed = Date.now() - startTime;
          if (elapsed > 35000) { // Give a bit more time than the Rust timeout
            setResult(`⏰ Monitoring timeout after ${currentCount} polls`);
            setIsPolling(false);
            pollingStateRef.current = null;
            return;
          }

          // Wait 1 second before checking again
          await new Promise(resolve => setTimeout(resolve, 1000));
        }

        if (pollingState.isCancelled()) {
          const finalCount = pollingState.getPollCount();
          setResult(`🛑 Cancelled after ${finalCount} polls`);
          setIsPolling(false);
          pollingStateRef.current = null;
        }
      };

      // Start monitoring the polling state
      monitorPolling().catch((error) => {
        console.error('❌ Monitoring Error:', error);
        if (error.toString().includes('BigInt')) {
          setResult(`❌ Monitoring Error: BigInt serialization issue - ${error}`);
        } else {
          setResult(`❌ Monitoring Error: ${error}`);
        }
        setIsPolling(false);
        pollingStateRef.current = null;
      });

    } catch (error) {
      console.log('❌ Error starting NWC polling:', error);
      setResult(`❌ Error: ${error}`);
      setIsPolling(false);
      pollingStateRef.current = null;
    }
  };

  const cancelPolling = () => {
    console.log('🛑 Cancel button clicked...');
    if (pollingStateRef.current && !pollingStateRef.current.isCancelled()) {
      console.log('🛑 Cancelling polling...');
      pollingStateRef.current.cancel();
      setResult('🛑 Cancel requested!');
      
      // The monitoring loop will detect the cancellation and clean up
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
        setResult('✅ LNI library loaded successfully! Ready to test InvoicePollingState approach.');
      } catch (error) {
        console.error('Error initializing LNI library:', error);
        setResult(`⚠️ Library loaded, but LND connection failed (expected): ${error}`);
      }
    };
    runRustCode();
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.title}>InvoicePollingState NWC Test</Text>
      <Text style={styles.result}>{result}</Text>
      
      <View style={styles.buttonContainer}>
        <Button
          title="Start NWC Polling"
          onPress={testNwcPolling}
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
