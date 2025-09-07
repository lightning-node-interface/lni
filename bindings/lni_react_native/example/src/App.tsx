import { useEffect, useState, useRef } from 'react';
import { Text, View, StyleSheet, Button, ScrollView, SafeAreaView, Alert } from 'react-native';
import {
  LndNode,
  LndConfig,
  PhoenixdNode,
  PhoenixdConfig,
  ClnNode,
  ClnConfig,
  StrikeNode,
  StrikeConfig,
  BlinkNode,
  BlinkConfig,
  NwcNode,
  NwcConfig,
  type OnInvoiceEventCallback,
  Transaction,
  OnInvoiceEventParams,
  InvoiceType,
  CreateInvoiceParams,
  LookupInvoiceParams,
} from 'lni_react_native';
import { 
  LND_URL, 
  LND_MACAROON, 
  LND_TEST_PAYMENT_HASH,
  CLN_URL,
  CLN_RUNE,
  CLN_TEST_PAYMENT_HASH,
  PHOENIXD_URL,
  PHOENIXD_PASSWORD,
  PHOENIXD_TEST_PAYMENT_HASH,
  STRIKE_API_KEY,
  STRIKE_TEST_PAYMENT_HASH,
  NWC_URI,
  NWC_TEST_PAYMENT_HASH,
  BLINK_API_KEY,
  BLINK_TEST_PAYMENT_HASH
} from '@env';

type TestResult = {
  implementation: string;
  status: 'running' | 'success' | 'error' | 'skipped';
  output: string[];
  startTime: number;
  endTime?: number;
};

export default function App() {
  const [testResults, setTestResults] = useState<TestResult[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [currentTest, setCurrentTest] = useState<string>('');

  // Helper function to safely serialize objects with BigInt values
  const safeStringify = (obj: any, indent = 2) => {
    return JSON.stringify(obj, (key, value) => {
      if (typeof value === 'bigint') {
        return value.toString();
      }
      return value;
    }, indent);
  };

  // Add output to current test result
  const addOutput = (implementation: string, message: string) => {
    setTestResults(prev => 
      prev.map(result => 
        result.implementation === implementation 
          ? { ...result, output: [...result.output, `[${new Date().toLocaleTimeString()}] ${message}`] }
          : result
      )
    );
  };

  // Update test status
  const updateTestStatus = (implementation: string, status: 'running' | 'success' | 'error' | 'skipped', endMessage?: string) => {
    setTestResults(prev => 
      prev.map(result => 
        result.implementation === implementation 
          ? { 
              ...result, 
              status, 
              endTime: status !== 'running' ? Date.now() : result.endTime,
              output: endMessage ? [...result.output, `[${new Date().toLocaleTimeString()}] ${endMessage}`] : result.output
            }
          : result
      )
    );
  };

  // Shared test logic for async node implementations
  const testAsyncNode = async (nodeName: string, node: any, testInvoiceHash?: string) => {
    try {
      addOutput(nodeName, `Testing ${nodeName}...`);
      
      // Test 1: Get node info
      addOutput(nodeName, '(1) Testing getInfo...');
      const info = await node.getInfo();
      addOutput(nodeName, `Node info: ${info.alias} (${info.pubkey?.substring(0, 20)}...)`);

      // Test 2: Create invoice
      addOutput(nodeName, '(2) Testing createInvoice...');
      const invoice = await node.createInvoice({
        amountMsats: BigInt(1000),
        description: `test invoice from ${nodeName}`,
        invoiceType: InvoiceType.Bolt11,
      });
      addOutput(nodeName, `Invoice created: ${invoice.paymentHash?.substring(0, 20)}...`);

      // Test 3: Lookup invoice (if test hash provided)
      if (testInvoiceHash) {
        addOutput(nodeName, '(3) Testing lookupInvoice...');
        try {
          const lookupInvoice = await node.lookupInvoice({
            paymentHash: testInvoiceHash,
            search: '',
          });
          addOutput(nodeName, `Lookup success: ${lookupInvoice.paymentHash?.substring(0, 20)}...`);
        } catch (error) {
          addOutput(nodeName, `Lookup failed (expected if hash doesn't exist): ${error}`);
        }
      }

      // Test 4: List transactions
      addOutput(nodeName, '(4) Testing listTransactions...');
      const txns = await node.listTransactions({
        from: BigInt(0),
        limit: BigInt(3),
      });
      addOutput(nodeName, `Found ${txns.length} transactions`);

      // Test 5: Decode invoice (if we have one)
      if (invoice?.invoice) {
        addOutput(nodeName, '(5) Testing decode...');
        try {
          const decoded = await node.decode(invoice.invoice);
          addOutput(nodeName, `Decode success: ${decoded.substring(0, 50)}...`);
        } catch (error) {
          addOutput(nodeName, `Decode failed: ${error}`);
        }
      }

      // Test 6: Invoice Events (callback-style)
      if (testInvoiceHash) {
        addOutput(nodeName, '(6) Testing onInvoiceEvents...');
        try {
          const params = OnInvoiceEventParams.create({
            paymentHash: testInvoiceHash,
            search: '',
            pollingDelaySec: BigInt(2),
            maxPollingSec: BigInt(10)
          });

          const callback: OnInvoiceEventCallback = {
            success: (transaction) => {
              addOutput(nodeName, `Invoice event SUCCESS: ${transaction?.paymentHash?.substring(0, 20)}...`);
            },
            pending: (transaction) => {
              addOutput(nodeName, `Invoice event PENDING: ${transaction ? 'transaction found' : 'no transaction'}`);
            },
            failure: (transaction) => {
              addOutput(nodeName, `Invoice event FAILURE: ${transaction?.paymentHash?.substring(0, 20) || 'no transaction'}`);
            }
          };

          await node.onInvoiceEvents(params, callback);
          addOutput(nodeName, 'Invoice events test completed');
        } catch (error) {
          addOutput(nodeName, `Invoice events failed: ${error}`);
        }
      }

      updateTestStatus(nodeName, 'success', `${nodeName} tests completed successfully!`);

    } catch (error) {
      console.error(`${nodeName} test error:`, error);
      updateTestStatus(nodeName, 'error', `${nodeName} test failed: ${error}`);
    }
  };

  // Individual test implementations
  const testLnd = async () => {
    const nodeName = 'LND';
    
    if (!LND_URL || !LND_MACAROON) {
      updateTestStatus(nodeName, 'skipped', 'LND_URL or LND_MACAROON not configured');
      return;
    }

    try {
      const config = LndConfig.create({
        url: LND_URL,
        macaroon: LND_MACAROON,
        socks5Proxy: '',
        acceptInvalidCerts: true,
        httpTimeout: BigInt(40)
      });

      const node = new LndNode(config);
      await testAsyncNode(nodeName, node, LND_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `LND initialization failed: ${error}`);
    }
  };

  const testCln = async () => {
    const nodeName = 'CLN';
    
    if (!CLN_URL || !CLN_RUNE) {
      updateTestStatus(nodeName, 'skipped', 'CLN_URL or CLN_RUNE not configured');
      return;
    }

    try {
      const config = ClnConfig.create({
        url: CLN_URL,
        rune: CLN_RUNE,
        socks5Proxy: '',
        acceptInvalidCerts: true,
        httpTimeout: BigInt(40)
      });

      const node = new ClnNode(config);
      await testAsyncNode(nodeName, node, CLN_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `CLN initialization failed: ${error}`);
    }
  };

  const testStrike = async () => {
    const nodeName = 'Strike';
    
    if (!STRIKE_API_KEY) {
      updateTestStatus(nodeName, 'skipped', 'STRIKE_API_KEY not configured');
      return;
    }

    try {
      const config = StrikeConfig.create({
        apiKey: STRIKE_API_KEY,
        socks5Proxy: '',
      });

      const node = new StrikeNode(config);
      await testAsyncNode(nodeName, node, STRIKE_TEST_PAYMENT_HASH); // No STRIKE_TEST_PAYMENT_HASH available
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Strike initialization failed: ${error}`);
    }
  };

  const testPhoenixd = async () => {
    const nodeName = 'Phoenixd';
    
    if (!PHOENIXD_URL || !PHOENIXD_PASSWORD) {
      updateTestStatus(nodeName, 'skipped', 'PHOENIXD_URL or PHOENIXD_PASSWORD not configured');
      return;
    }

    try {
      const config = PhoenixdConfig.create({
        url: PHOENIXD_URL,
        password: PHOENIXD_PASSWORD,
        socks5Proxy: '',
        acceptInvalidCerts: true,
        httpTimeout: BigInt(40)
      });

      const node = new PhoenixdNode(config);
      
      // Phoenixd has slightly different method names, so we'll do a custom test
      addOutput(nodeName, 'Testing Phoenixd...');
      
      addOutput(nodeName, 'Testing getInfo...');
      const info = await node.getInfo();
      addOutput(nodeName, `Node info: ${info.alias} (${info.pubkey?.substring(0, 20)}...)`);

      addOutput(nodeName, 'Testing createInvoice...');
      const invoice = await node.createInvoice(
        CreateInvoiceParams.create({
          invoiceType: InvoiceType.Bolt11,
          amountMsats: BigInt(1000),
          offer: undefined,
          description: 'test invoice from Phoenixd',
          descriptionHash: undefined,
          expiry: undefined,
          rPreimage: undefined,
          isBlinded: undefined,
          isKeysend: undefined,
          isAmp: undefined,
          isPrivate: undefined,
        })
      );
      addOutput(nodeName, `Invoice created: ${invoice.paymentHash?.substring(0, 20)}...`);

      if (PHOENIXD_TEST_PAYMENT_HASH) {
        addOutput(nodeName, 'Testing lookupInvoice...');
        try {
          const lookupInvoice = await node.lookupInvoice(
            LookupInvoiceParams.create({
              paymentHash: PHOENIXD_TEST_PAYMENT_HASH,
              search: undefined,
            })
          );
          addOutput(nodeName, `Lookup success: ${lookupInvoice.paymentHash?.substring(0, 20)}...`);
        } catch (error) {
          addOutput(nodeName, `Lookup failed: ${error}`);
        }
      }

      updateTestStatus(nodeName, 'success', 'Phoenixd tests completed successfully!');
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Phoenixd test failed: ${error}`);
    }
  };

  const testBlink = async () => {
    const nodeName = 'Blink';
    
    if (!BLINK_API_KEY) {
      updateTestStatus(nodeName, 'skipped', 'BLINK_API_KEY not configured');
      return;
    }

    try {
      const config = BlinkConfig.create({
        apiKey: BLINK_API_KEY,
      });

      const node = new BlinkNode(config);
      await testAsyncNode(nodeName, node, BLINK_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Blink initialization failed: ${error}`);
    }
  };

  const testNwc = async () => {
    const nodeName = 'NWC';
    
    if (!NWC_URI) {
      updateTestStatus(nodeName, 'skipped', 'NWC_URI not configured');
      return;
    }

    try {
      const config = NwcConfig.create({
        nwcUri: NWC_URI,
      });

      const node = new NwcNode(config);
      await testAsyncNode(nodeName, node, NWC_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `NWC initialization failed: ${error}`);
    }
  };

  // Initialize test result for an implementation
  const initializeTest = (implementation: string) => {
    setTestResults(prev => {
      const existing = prev.find(r => r.implementation === implementation);
      if (existing) {
        return prev.map(r => 
          r.implementation === implementation 
            ? { ...r, status: 'running' as const, output: [], startTime: Date.now(), endTime: undefined }
            : r
        );
      } else {
        return [...prev, {
          implementation,
          status: 'running' as const,
          output: [],
          startTime: Date.now()
        }];
      }
    });
  };

  // Run individual test
  const runTest = async (implementation: string, testFn: () => Promise<void>) => {
    if (isRunning) return;
    
    setIsRunning(true);
    setCurrentTest(implementation);
    initializeTest(implementation);
    
    try {
      await testFn();
    } catch (error) {
      updateTestStatus(implementation, 'error', `Unexpected error: ${error}`);
    } finally {
      setIsRunning(false);
      setCurrentTest('');
    }
  };

  // Run all tests
  const runAllTests = async () => {
    if (isRunning) return;
    
    setIsRunning(true);
    setTestResults([]);
    
    const tests = [
      { name: 'LND', fn: testLnd },
      { name: 'CLN', fn: testCln },
      { name: 'Strike', fn: testStrike },
      { name: 'Phoenixd', fn: testPhoenixd },
      { name: 'Blink', fn: testBlink },
      { name: 'NWC', fn: testNwc },
    ];

    for (const test of tests) {
      setCurrentTest(test.name);
      initializeTest(test.name);
      
      try {
        await test.fn();
        // Small delay between tests
        await new Promise(resolve => setTimeout(resolve, 500));
      } catch (error) {
        updateTestStatus(test.name, 'error', `Unexpected error: ${error}`);
      }
    }
    
    setIsRunning(false);
    setCurrentTest('');
  };

  // Clear results
  const clearResults = () => {
    setTestResults([]);
  };

  // Get status color
  const getStatusColor = (status: string) => {
    switch (status) {
      case 'running': return '#FFA500';
      case 'success': return '#008000';
      case 'error': return '#FF0000';
      case 'skipped': return '#808080';
      default: return '#000000';
    }
  };

  // Get status icon
  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'running': return '🔄';
      case 'success': return '✅';
      case 'error': return '❌';
      case 'skipped': return '⏭️';
      default: return '⏸️';
    }
  };

  return (
    <SafeAreaView style={styles.container}>
      <Text style={styles.title}>Lightning Node Interface Tests</Text>
      
      {/* Control Buttons */}
      <View style={styles.controlsContainer}>
        <View style={styles.buttonRow}>
          <Button 
            title="Run All Tests" 
            onPress={runAllTests} 
            disabled={isRunning}
            color="#4CAF50"
          />
          <Button 
            title="Clear Results" 
            onPress={clearResults} 
            disabled={isRunning}
            color="#FF9800"
          />
           <Button 
            title="UX Test" 
            onPress={() => Alert.alert('Button pressed')} 
            color="blue"
          />
        </View>
        
        <View style={styles.buttonRow}>
          <Button 
            title="LND" 
            onPress={() => runTest('LND', testLnd)} 
            disabled={isRunning}
            color="#FF6B6B"
          />
          <Button 
            title="CLN" 
            onPress={() => runTest('CLN', testCln)} 
            disabled={isRunning}
            color="#4ECDC4"
          />
          <Button 
            title="Strike" 
            onPress={() => runTest('Strike', testStrike)} 
            disabled={isRunning}
            color="#45B7D1"
          />
        </View>
        
        <View style={styles.buttonRow}>
          <Button 
            title="Phoenixd" 
            onPress={() => runTest('Phoenixd', testPhoenixd)} 
            disabled={isRunning}
            color="#96CEB4"
          />
          <Button 
            title="Blink" 
            onPress={() => runTest('Blink', testBlink)} 
            disabled={isRunning}
            color="#FFEAA7"
          />
          <Button 
            title="NWC" 
            onPress={() => runTest('NWC', testNwc)} 
            disabled={isRunning}
            color="#DDA0DD"
          />
        </View>
      </View>

      {/* Current Test Indicator */}
      {isRunning && (
        <Text style={styles.currentTest}>
          Currently running: {currentTest}
        </Text>
      )}

      {/* Results */}
      <ScrollView style={styles.resultsContainer} showsVerticalScrollIndicator={true}>
        {testResults.map((result) => (
          <View key={result.implementation} style={styles.resultItem}>
            <View style={styles.resultHeader}>
              <Text style={[styles.resultTitle, { color: getStatusColor(result.status) }]}>
                {getStatusIcon(result.status)} {result.implementation}
              </Text>
              <Text style={styles.resultTime}>
                {result.endTime && result.startTime ? 
                  `${((result.endTime - result.startTime) / 1000).toFixed(1)}s` : 
                  'Running...'}
              </Text>
            </View>
            
            <ScrollView style={styles.outputContainer} nestedScrollEnabled={true}>
              {result.output.map((line, index) => (
                <Text key={index} style={styles.outputLine}>
                  {line}
                </Text>
              ))}
            </ScrollView>
          </View>
        ))}
      </ScrollView>

      {/* Instructions */}
      {testResults.length === 0 && !isRunning && (
        <View style={styles.instructionsContainer}>
          <Text style={styles.instructionsText}>
            Configure environment variables in your .env file:
          </Text>
          <Text style={styles.envText}>
            LND_URL, LND_MACAROON{'\n'}
            CLN_URL, CLN_RUNE{'\n'}
            STRIKE_API_KEY{'\n'}
            PHOENIXD_URL, PHOENIXD_PASSWORD{'\n'}
            And test payment hashes for each implementation
          </Text>
        </View>
      )}
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    padding: 16,
    backgroundColor: '#f5f5f5',
  },
  title: {
    fontSize: 24,
    fontWeight: 'bold',
    textAlign: 'center',
    marginBottom: 20,
    color: '#333',
  },
  controlsContainer: {
    marginBottom: 20,
  },
  buttonRow: {
    flexDirection: 'row',
    justifyContent: 'space-around',
    marginVertical: 8,
  },
  currentTest: {
    fontSize: 16,
    fontWeight: 'bold',
    textAlign: 'center',
    color: '#FF6B00',
    marginBottom: 10,
  },
  resultsContainer: {
    flex: 1,
    backgroundColor: '#fff',
    borderRadius: 8,
    padding: 12,
  },
  resultItem: {
    marginBottom: 16,
    borderWidth: 1,
    borderColor: '#ddd',
    borderRadius: 8,
    padding: 12,
    backgroundColor: '#fafafa',
  },
  resultHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 8,
  },
  resultTitle: {
    fontSize: 18,
    fontWeight: 'bold',
  },
  resultTime: {
    fontSize: 14,
    color: '#666',
  },
  outputContainer: {
    maxHeight: 500,
    backgroundColor: '#f9f9f9',
    borderRadius: 4,
    padding: 8,
  },
  outputLine: {
    fontSize: 12,
    fontFamily: 'monospace',
    color: '#333',
    marginBottom: 2,
  },
  instructionsContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 20,
  },
  instructionsText: {
    fontSize: 16,
    textAlign: 'center',
    color: '#666',
    marginBottom: 12,
  },
  envText: {
    fontSize: 14,
    fontFamily: 'monospace',
    textAlign: 'center',
    color: '#888',
    backgroundColor: '#f0f0f0',
    padding: 12,
    borderRadius: 4,
  },
});
