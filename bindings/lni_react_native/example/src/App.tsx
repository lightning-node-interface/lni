import { useEffect, useState, useRef } from 'react';
import { Text, View, StyleSheet, Button, Alert, TextInput, Animated, InteractionManager } from 'react-native';
import { DeviceEventEmitter } from 'react-native';
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
  getInfoSync,
  onInvoiceEventsAsync,
  sayAfterWithTokio,
} from 'lni_react_native';
import { LND_URL, LND_MACAROON, LND_TEST_PAYMENT_HASH, NWC_URI, NWC_TEST_PAYMENT_HASH } from '@env';

export default function App() {
  const [result, setResult] = useState<string>('Ready to test UI thread blocking...');
  const [isPolling, setIsPolling] = useState(false);
  const [pollCount, setPollCount] = useState(0);
  const [uiCounter, setUiCounter] = useState(0);
  const [spinnerRotation] = useState(new Animated.Value(0));
  const [textInput, setTextInput] = useState('Type here to test UI responsiveness');
  const pollingStateRef = useRef<InvoicePollingStateInterface | null>(null);
  const lndNodeRef = useRef<LndNode | null>(null);
  const counterIntervalRef = useRef<NodeJS.Timeout | null>(null);

  // UI responsiveness test - counter that increments every second
  useEffect(() => {
    counterIntervalRef.current = setInterval(() => {
      setUiCounter(prev => prev + 1);
    }, 1000);

    // Start spinning animation
    Animated.loop(
      Animated.timing(spinnerRotation, {
        toValue: 1,
        duration: 1000,
        useNativeDriver: true,
      })
    ).start();

    return () => {
      if (counterIntervalRef.current) {
        clearInterval(counterIntervalRef.current);
      }
    };
  }, []);

  const spin = spinnerRotation.interpolate({
    inputRange: [0, 1],
    outputRange: ['0deg', '360deg'],
  });

  // Helper function to safely serialize objects with BigInt values
  const safetStringify = (obj: any, indent = 2) => {
    return JSON.stringify(obj, (key, value) => {
      if (typeof value === 'bigint') {
        return value.toString();
      }
      return value;
    }, indent);
  };

  const testLndAsync = async () => {
    try {
      // Validate environment variables
      if (!LND_URL || !LND_MACAROON) {
        setResult('❌ Error: LND_URL or LND_MACAROON not found in environment variables');
        return;
      }

      setResult('🔄 Testing LND sync with background processing (15s delay)...');

      const config = LndConfig.create({
        url: LND_URL,
        macaroon: LND_MACAROON,
        socks5Proxy: '', // empty string instead of undefined
        acceptInvalidCerts: true,
      });

      console.log('🔧 Testing LND sync functionality with background processing');
      console.log('🔧 Using LND_URL:', LND_URL);
      console.log('🔧 Using LND_MACAROON:', LND_MACAROON.substring(0, 20) + '...');

      console.log('📋 Calling lndGetInfoSync() with multiple deferrals...');
      const startTime = Date.now();

      // Try multiple levels of deferral to force background execution
      const nodeInfo = await new Promise<any>((resolve, reject) => {
        // First deferral
        setTimeout(() => {
          // Second deferral using requestIdleCallback if available, or setTimeout
          const deferAgain = () => {
            setTimeout(() => {
              try {
                console.log('🔧 Executing synchronous LND call on deferred thread...');
                const result = getInfoSync(config);
                console.log('🔧 Synchronous call completed');
                resolve(result);
              } catch (error) {
                console.error('🔧 Synchronous call failed:', error);
                reject(error);
              }
            }, 0);
          };

          // Try to use requestIdleCallback for even more deferral
          deferAgain();
        }, 0);
      });

      const endTime = Date.now();

      console.log('✅ LND sync response received:', safetStringify(nodeInfo));
      console.log(`⏱️ API call took ${endTime - startTime}ms`);

      setResult(`✅ LND Sync Success! (${endTime - startTime}ms)
Node: ${nodeInfo.alias || 'Unknown'}
Pubkey: ${nodeInfo.pubkey.substring(0, 20)}...
Network: ${nodeInfo.network}
Block Height: ${nodeInfo.blockHeight}
Send Balance: ${nodeInfo.sendBalanceMsat} msat
Receive Balance: ${nodeInfo.receiveBalanceMsat} msat

🔍 UI Test Results:
• Counter should have continued incrementing
• Spinner should have kept spinning
• Buttons should remain responsive
• Text input should be editable`);

    } catch (error) {
      console.error('❌ LND sync test error:', error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      if (errorMessage.includes('connection refused') || errorMessage.includes('timeout')) {
        setResult(`❌ LND Connection Error: Could not connect to LND at ${LND_URL}. Please check your LND node is running and accessible.`);
      } else if (errorMessage.includes('authentication') || errorMessage.includes('macaroon')) {
        setResult(`❌ LND Auth Error: Invalid macaroon. Please check your LND_MACAROON environment variable.`);
      } else {
        setResult(`❌ LND Sync Error: ${errorMessage}`);
      }
    }
  };

  const testLndInvoiceEvents = async () => {
    try {
      // Validate environment variables
      if (!LND_URL || !LND_MACAROON) {
        setResult('❌ Error: LND_URL or LND_MACAROON not found in environment variables');
        return;
      }
      if (!LND_TEST_PAYMENT_HASH) {
        setResult('❌ Error: LND_TEST_PAYMENT_HASH not found in environment variables');
        return;
      }

      setResult('🔄 Testing LND async invoice events...');
      setIsPolling(true);
      setPollCount(0);
      
      // Clear any NWC reference
      pollingStateRef.current = null;

      const config = LndConfig.create({
        url: LND_URL,
        macaroon: LND_MACAROON,
        socks5Proxy: '', // empty string instead of undefined
        acceptInvalidCerts: true,
      });

      const node = new LndNode(config);
      lndNodeRef.current = node; // Store reference for potential cancellation

      const params = OnInvoiceEventParams.create({
        paymentHash: LND_TEST_PAYMENT_HASH,
        search: undefined,
        pollingDelaySec: BigInt(3),
        maxPollingSec: BigInt(20),
      });

      console.log('🔧 Starting LND async invoice events test');
      console.log('🔧 Using LND_URL:', LND_URL);
      console.log('🔧 Using payment hash:', LND_TEST_PAYMENT_HASH);

      // Create callback to handle events with simpler structure
      const handleSuccess = (transaction: Transaction | undefined) => {
        console.log('✅ LND Success callback:', transaction);
        setResult(`✅ LND Invoice Event Success! Transaction: ${transaction ? safetStringify(transaction).substring(0, 200) + '...' : 'No transaction data'}`);
        setIsPolling(false);
        lndNodeRef.current = null;
      };

      const handlePending = (transaction: Transaction | undefined) => {
        const count = pollCount + 1;
        setPollCount(count);
        console.log(`🔄 LND Pending callback #${count}:`, transaction);
        setResult(`🔄 LND Poll #${count}: Invoice pending... ${transaction ? 'Transaction found' : 'No transaction yet'}`);
      };

      const handleFailure = (transaction: Transaction | undefined) => {
        console.log(`❌ LND Failure callback ${Date.now()}:`, transaction);
        //setResult(`❌ LND Invoice Event Failed. ${transaction ? 'Transaction: ' + safetStringify(transaction).substring(0, 100) + '...' : 'No transaction data'}`);
        //setIsPolling(false);
        lndNodeRef.current = null;
      };

      const callback: OnInvoiceEventCallback = {
        success: handleSuccess,
        pending: handlePending,
        failure: handleFailure,
      };

      console.log('📋 Starting LND async invoice events with config:', safetStringify(config));
      console.log('📋 Params:', safetStringify(params));
      console.log('📋 Callback:', callback);
      console.log('📋 Available onInvoiceEventsAsync:', typeof onInvoiceEventsAsync);

      // Start the async invoice event monitoring using the direct function
      try {
        console.log('📋 Calling onInvoiceEventsAsync...');
        const result = await onInvoiceEventsAsync(config, params, callback);
        console.log('🔄 LND async invoice events completed:', result);
        if (isPolling) {
          setResult('🔄 LND async invoice events monitoring completed');
          setIsPolling(false);
          lndNodeRef.current = null;
        }
      } catch (error) {
        console.error('❌ LND async invoice events error:', error);
        console.error('❌ Error type:', typeof error);
        console.error('❌ Error constructor:', error?.constructor?.name);
        setResult(`❌ LND Async Invoice Events Error: ${error}`);
        setIsPolling(false);
        lndNodeRef.current = null;
      }

      // Set a timeout to cancel if it takes too long
      setTimeout(() => {
        if (isPolling) {
          setResult('⏰ LND async invoice events test timeout (25s)');
          setIsPolling(false);
          lndNodeRef.current = null;
        }
      }, 25000);

    } catch (error) {
      console.error('❌ Error starting LND invoice events test:', error);
      setResult(`❌ LND Test Error: ${error}`);
      setIsPolling(false);
      lndNodeRef.current = null;
    }
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
      
      // Clear any LND reference 
      lndNodeRef.current = null;

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
    
    let cancelledSomething = false;
    
    // Cancel NWC polling if active
    if (pollingStateRef.current && !pollingStateRef.current.isCancelled()) {
      console.log('🛑 Cancelling NWC polling...');
      pollingStateRef.current.cancel();
      cancelledSomething = true;
    }
    
    // Cancel LND polling if active 
    if (lndNodeRef.current) {
      console.log('🛑 Cancelling LND polling...');
      lndNodeRef.current = null; // Clear reference to signal cancellation intent
      cancelledSomething = true;
    }
    
    if (cancelledSomething) {
      setResult('🛑 Cancel requested!');
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
      <Text style={styles.title}>UI Thread Blocking Test</Text>
      
      {/* UI Responsiveness Indicators */}
      <View style={styles.indicatorsContainer}>
        <View style={styles.indicator}>
          <Animated.Text style={[styles.spinner, { transform: [{ rotate: spin }] }]}>
            🔄
          </Animated.Text>
          <Text style={styles.indicatorText}>Spinner: {uiCounter}</Text>
        </View>
        
        <TextInput
          style={styles.textInput}
          value={textInput}
          onChangeText={setTextInput}
          placeholder="Type here during API call..."
        />
      </View>

      <Text style={styles.result}>{result}</Text>
      
      <View style={styles.buttonContainer}>
        <Button
          title="Test LND Sync (15s delay)"
          onPress={testLndAsync}
          color="green"
        />
        
        <Button
          title="UI Test Button 1"
          onPress={() => Alert.alert('Button 1', `Counter: ${uiCounter}`)}
          color="blue"
        />
        
        <Button
          title="UI Test Button 2" 
          onPress={() => Alert.alert('Button 2', `Text: ${textInput}`)}
          color="orange"
        />
      </View>
      
      <View style={styles.buttonContainer}>
        <Button
          title="Start NWC Polling"
          onPress={testNwcPolling}
          disabled={isPolling}
        />
        
        <Button
          title="Test LND Async Events"
          onPress={testLndInvoiceEvents}
          disabled={isPolling}
          color="purple"
        />
        
        <Button
          title="Cancel Polling"
          onPress={cancelPolling}
          disabled={!isPolling}
          color="red"
        />
      </View>

      <Button
          title="Say After Tokio"
          onPress={()=>{
            const say = sayAfterWithTokio(5000, "World", "https://.onion:8080/v1/getinfo", "socks5h://localhost:9050", "header_key", "header_value").then(setResult);
          }}
          color="red"
        />
      <Button
          title="Say After"
          onPress={()=>{
            Alert.alert('Say After', 'Hello, World!');
          }}
          color="pink"
        />
      
      <View style={styles.statusContainer}>
        <Text style={styles.statusText}>
          Status: {isPolling ? `🔄 Polling... (${pollCount} attempts)` : '⏸️ Stopped'}
        </Text>
        <Text style={styles.helperText}>
          {isPolling 
            ? "🔥 UI should remain responsive! Try tapping buttons." 
            : "Test async functions to see non-blocking behavior."
          }
        </Text>
        <Text style={styles.helperText}>
          Watch the spinner and counter - they should keep moving if UI thread is not blocked!
        </Text>
        <Text style={styles.helperText}>
          Active polling: {pollingStateRef.current ? 'NWC' : lndNodeRef.current ? 'LND' : 'None'}
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
  indicatorsContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    marginBottom: 20,
    padding: 10,
    backgroundColor: '#f0f0f0',
    borderRadius: 10,
  },
  indicator: {
    alignItems: 'center',
    marginHorizontal: 20,
  },
  spinner: {
    fontSize: 30,
    marginBottom: 5,
  },
  indicatorText: {
    fontSize: 12,
    color: '#666',
  },
  textInput: {
    flex: 1,
    borderWidth: 1,
    borderColor: '#ccc',
    borderRadius: 5,
    padding: 10,
    marginLeft: 10,
    backgroundColor: 'white',
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
