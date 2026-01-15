import { useEffect, useState, useRef } from 'react';
import { Text, View, StyleSheet, Button, ScrollView, SafeAreaView, Alert, TextInput, Switch, Platform } from 'react-native';
import RNFS from 'react-native-fs';
import {
  LndConfig,
  PhoenixdConfig,
  ClnConfig,
  StrikeConfig,
  BlinkConfig,
  NwcConfig,
  SpeedConfig,
  SparkConfig,
  type OnInvoiceEventCallback,
  Transaction,
  OnInvoiceEventParams,
  InvoiceType,
  CreateInvoiceParams,
  LookupInvoiceParams,
  ListTransactionsParams,
  CreateOfferParams,
  Offer,
  // Polymorphic interface and factory functions
  type LightningNode,
  createLndNode,
  createClnNode,
  createStrikeNode,
  createPhoenixdNode,
  createBlinkNode,
  createNwcNode,
  createSpeedNode,
  generateMnemonic,
  createSparkNode,
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
  BLINK_TEST_PAYMENT_HASH,
  SPEED_API_KEY,
  SPEED_TEST_PAYMENT_HASH,
  SPARK_MNEMONIC,
  SPARK_API_KEY,
  SPARK_TEST_PAYMENT_HASH
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
  const [socks5ProxyEnabled, setSocks5ProxyEnabled] = useState(false);
  const [socks5ProxyUrl, setSocks5ProxyUrl] = useState('socks5h://localhost:9050');

  // Get the proxy value to use in configurations
  const getProxyValue = () => {
    return socks5ProxyEnabled ? socks5ProxyUrl : '';
  };

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

  // Shared test logic for any LightningNode implementation (polymorphic!)
  const testAsyncNode = async (nodeName: string, node: LightningNode, testInvoiceHash?: string) => {
    try {
      addOutput(nodeName, `Testing ${nodeName}...`);
      
      // Test 1: Get node info
      addOutput(nodeName, '(1) Testing getInfo...');
      const info = await node.getInfo();
      addOutput(nodeName, `Node info: ${info.alias} (${info.pubkey?.substring(0, 20)}...)`);

      // Test 2: Create invoice
      addOutput(nodeName, '(2) Testing createInvoice...');
      const invoiceParams = CreateInvoiceParams.create({
        amountMsats: BigInt(1000),
        description: `test invoice from ${nodeName}`,
        expiry: BigInt(3600),
      });
      const invoice = await node.createInvoice(invoiceParams);
      addOutput(nodeName, `Invoice created: ${invoice.paymentHash?.substring(0, 20)}...`);

      // Test 3: Lookup invoice (if test hash provided)
      if (testInvoiceHash) {
        addOutput(nodeName, '(3) Testing lookupInvoice...');
        try {
          const lookupParams = LookupInvoiceParams.create({
            paymentHash: testInvoiceHash,
            search: undefined,
          });
          const lookupInvoice = await node.lookupInvoice(lookupParams);
          addOutput(nodeName, `Lookup success: ${lookupInvoice.paymentHash?.substring(0, 20)}...`);
        } catch (error) {
          addOutput(nodeName, `Lookup failed (expected if hash doesn't exist): ${error}`);
        }
      }

      // Test 4: List transactions
      addOutput(nodeName, '(4) Testing listTransactions...');
      const listParams = ListTransactionsParams.create({
        from: BigInt(0),
        limit: BigInt(3),
        paymentHash: undefined,
        search: undefined,
      });
      const txns = await node.listTransactions(listParams);
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

      // Test 6: Create BOLT12 Offer (if supported)      
      if (typeof node.createOffer === 'function') {
        addOutput(nodeName, '(6) Testing createOffer...');
        try {          
          // Test without amount (reusable offer)
          const offerParams = CreateOfferParams.create({
            description: `BOLT12 reusable offer from ${nodeName}`,
            amountMsats: undefined,
          });
          const reusableOffer = await node.createOffer(offerParams);
          addOutput(nodeName, `Reusable offer created: ${reusableOffer.bolt12?.substring(0, 30)}...`);
        } catch (error) {
          addOutput(nodeName, `createOffer failed (may not be supported): ${error}`);
        }
      } else {
        addOutput(nodeName, '(6) createOffer not supported by this implementation');
      }
      

      // Test 7: Invoice Events (callback-style)
      if (testInvoiceHash) {
        addOutput(nodeName, '(7) Testing onInvoiceEvents...');
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
        socks5Proxy: getProxyValue(),
        acceptInvalidCerts: true,
        httpTimeout: BigInt(40)
      });

      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createLndNode(config);
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
        socks5Proxy: getProxyValue(),
        acceptInvalidCerts: true,
        httpTimeout: BigInt(40)
      });

      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createClnNode(config);
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
        socks5Proxy: getProxyValue(),
      });

      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createStrikeNode(config);
      await testAsyncNode(nodeName, node, STRIKE_TEST_PAYMENT_HASH);
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
        socks5Proxy: getProxyValue(),
        // Phoenixd often needs a bit more time
        acceptInvalidCerts: true,
        httpTimeout: BigInt(60)
      });
      
      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createPhoenixdNode(config);
      await testAsyncNode(nodeName, node, PHOENIXD_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Phoenixd initialization failed: ${error}`);
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
        socks5Proxy: getProxyValue(),
      });

      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createBlinkNode(config);
      await testAsyncNode(nodeName, node, BLINK_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Blink initialization failed: ${error}`);
    }
  };

  const testSpeed = async () => {
    const nodeName = 'Speed';

    if (!SPEED_API_KEY) {
      updateTestStatus(nodeName, 'skipped', 'SPEED_API_KEY not configured');
      return;
    }

    try {
      const config = SpeedConfig.create({
        apiKey: SPEED_API_KEY,
        socks5Proxy: getProxyValue(),
      });

      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createSpeedNode(config);
      await testAsyncNode(nodeName, node, SPEED_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Speed initialization failed: ${error}`);
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
        socks5Proxy: getProxyValue(),
      });

      // Use factory function for polymorphic LightningNode
      const node: LightningNode = createNwcNode(config);
      await testAsyncNode(nodeName, node, NWC_TEST_PAYMENT_HASH);
    } catch (error) {
      updateTestStatus(nodeName, 'error', `NWC initialization failed: ${error}`);
    }
  };


  const testMnemonic = async () => {
    const nodeName = 'Mnemonic';

    try {
      addOutput(nodeName, 'Testing generateMnemonic...');

      // Test 12-word mnemonic (default)
      addOutput(nodeName, '(1) Generating 12-word mnemonic...');
      const mnemonic12 = generateMnemonic(12);
      const words12 = mnemonic12.split(' ');
      addOutput(nodeName, `12-word mnemonic: ${mnemonic12}`);
      addOutput(nodeName, `Word count: ${words12.length}`);

      // Test 24-word mnemonic
      addOutput(nodeName, '(2) Generating 24-word mnemonic...');
      const mnemonic24 = generateMnemonic(24);
      const words24 = mnemonic24.split(' ');
      addOutput(nodeName, `24-word mnemonic: ${mnemonic24}`);
      addOutput(nodeName, `Word count: ${words24.length}`);

      // Verify word counts
      if (words12.length === 12 && words24.length === 24) {
        updateTestStatus(nodeName, 'success', 'Mnemonic generation tests passed!');
      } else {
        updateTestStatus(nodeName, 'error', `Unexpected word counts: 12-word=${words12.length}, 24-word=${words24.length}`);
      }
    } catch (error) {
      updateTestStatus(nodeName, 'error', `Mnemonic test failed: ${error}`);
    }
  };

  const testSpark = async () => {
    const nodeName = 'Spark';

    if (!SPARK_MNEMONIC || !SPARK_API_KEY) {
      updateTestStatus(nodeName, 'skipped', 'SPARK_MNEMONIC or SPARK_API_KEY not configured');
      return;
    }

    // let sparkNode: SparkNode | null = null;

    try {
      addOutput(nodeName, 'Testing Spark node...');

      // Test 1: Create and connect SparkNode
      addOutput(nodeName, '(1) Creating SparkNode...');
      const storageDir = `${RNFS.DocumentDirectoryPath}/spark_data`;
      addOutput(nodeName, `Using storage directory: ${storageDir}`);
      const config = SparkConfig.create({
        mnemonic: SPARK_MNEMONIC,
        passphrase: undefined,
        apiKey: SPARK_API_KEY,
        storageDir: storageDir,
        network: 'mainnet',
      });

      const sparkNode: LightningNode = await createSparkNode(config);
      addOutput(nodeName, 'SparkNode connected successfully!');

      // Test 1: Get node info
      addOutput(nodeName, '(1) Testing getInfo...');
      const info = await sparkNode.getInfo();
      addOutput(nodeName, `Node info: ${info.alias} (balance: ${info.sendBalanceMsat} msats)`);

      // Test 2: Create invoice
      addOutput(nodeName, '(2) Testing createInvoice...');
      const invoiceParams = CreateInvoiceParams.create({
        amountMsats: BigInt(1000),
        description: 'test invoice from Spark',
        expiry: BigInt(3600),
      });
      const invoice = await sparkNode.createInvoice(invoiceParams);
      addOutput(nodeName, `Invoice created: ${invoice.paymentHash?.substring(0, 20)}...`);

      // Test 3: List transactions
      addOutput(nodeName, '(3) Testing listTransactions...');
      const listParams = ListTransactionsParams.create({
        from: BigInt(0),
        limit: BigInt(3),
        paymentHash: undefined,
        search: undefined,
      });
      const txns = await sparkNode.listTransactions(listParams);
      addOutput(nodeName, `Found ${txns.length} transactions`);

      // Test 4: Lookup invoice (if test hash provided)
      if (SPARK_TEST_PAYMENT_HASH) {
        addOutput(nodeName, '(4) Testing lookupInvoice...');
        try {
          const lookupParams = LookupInvoiceParams.create({
            paymentHash: SPARK_TEST_PAYMENT_HASH,
            search: undefined,
          });
          const lookupInvoice = await sparkNode.lookupInvoice(lookupParams);
          addOutput(nodeName, `Lookup success: ${lookupInvoice.paymentHash?.substring(0, 20)}...`);
        } catch (error) {
          addOutput(nodeName, `Lookup failed (expected if hash doesn't exist): ${error}`);
        }
      }

      // Test 5: Test onInvoiceEvents with callback
      addOutput(nodeName, '(5) Testing onInvoiceEvents...');
      try {
        const eventParams = OnInvoiceEventParams.create({
          paymentHash: SPARK_TEST_PAYMENT_HASH,
          search: undefined,
          pollingDelaySec: BigInt(1),
          maxPollingSec: BigInt(6),
        });

        const invoiceCallback: OnInvoiceEventCallback = {
          success: (transaction: Transaction | undefined) => {
            addOutput(nodeName, `onInvoiceEvents: SUCCESS - ${transaction?.paymentHash?.substring(0, 20) ?? 'no transaction'}...`);
          },
          pending: (transaction: Transaction | undefined) => {
            addOutput(nodeName, `onInvoiceEvents: PENDING - ${transaction?.paymentHash?.substring(0, 20) ?? 'no transaction'}...`);
          },
          failure: (transaction: Transaction | undefined) => {
            addOutput(nodeName, `onInvoiceEvents: FAILURE - ${transaction?.paymentHash?.substring(0, 20) ?? 'no transaction'}...`);
          },
        };

        await sparkNode.onInvoiceEvents(eventParams, invoiceCallback);
        addOutput(nodeName, 'onInvoiceEvents completed (timeout expected for unpaid invoice)');
      } catch (error) {
        addOutput(nodeName, `onInvoiceEvents error (expected for timeout): ${error}`);
      }

      updateTestStatus(nodeName, 'success', 'Spark tests completed successfully!');

    } catch (error) {
      console.error('Spark test error:', error);
      updateTestStatus(nodeName, 'error', `Spark test failed: ${error}`);
    } finally {
      // Disconnect from Spark
      // if (sparkNode) {
      //   try {
      //     await sparkNode.disconnect();
      //     addOutput(nodeName, 'Disconnected from Spark');
      //   } catch (e) {
      //     addOutput(nodeName, `Disconnect error: ${e}`);
      //   }
      // }
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
      
      {/* SOCKS5 Proxy Configuration */}
      <View style={styles.proxyContainer}>
        <View style={styles.proxyHeader}>
          <Text style={styles.proxyTitle}>SOCKS5 Proxy</Text>
          <Switch
            value={socks5ProxyEnabled}
            onValueChange={setSocks5ProxyEnabled}
            trackColor={{ false: '#767577', true: '#81b0ff' }}
            thumbColor={socks5ProxyEnabled ? '#f5dd4b' : '#f4f3f4'}
          />
        </View>
        <TextInput
          style={[styles.proxyInput, !socks5ProxyEnabled && styles.proxyInputDisabled]}
          value={socks5ProxyUrl}
          onChangeText={setSocks5ProxyUrl}
          placeholder="socks5h://localhost:9150"
          editable={socks5ProxyEnabled}
          autoCapitalize="none"
          autoCorrect={false}
        />
        <Text style={styles.proxyStatus}>
          Status: {socks5ProxyEnabled ? `Using proxy: ${socks5ProxyUrl}` : 'Direct connection (no proxy)'}
        </Text>
      </View>
      
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
            title="Speed" 
            onPress={() => runTest('Speed', testSpeed)} 
            disabled={isRunning}
            color="pink"
          />
          <Button 
            title="NWC" 
            onPress={() => runTest('NWC', testNwc)} 
            disabled={isRunning}
            color="#DDA0DD"
          />
          <Button 
            title="Mnemonic" 
            onPress={() => runTest('Mnemonic', testMnemonic)} 
            disabled={isRunning}
            color="#9B59B6"
          />
          <Button 
            title="Spark" 
            onPress={() => runTest('Spark', testSpark)} 
            disabled={isRunning}
            color="#FF8C00"
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
            And test payment hashes for each implementation{`\n`}
            SPARK_MNEMONIC, SPARK_API_KEY
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
  proxyContainer: {
    backgroundColor: '#fff',
    borderRadius: 8,
    padding: 16,
    marginBottom: 20,
    borderWidth: 1,
    borderColor: '#ddd',
  },
  proxyHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 12,
  },
  proxyTitle: {
    fontSize: 18,
    fontWeight: 'bold',
    color: '#333',
  },
  proxyInput: {
    borderWidth: 1,
    borderColor: '#ddd',
    borderRadius: 6,
    padding: 12,
    fontSize: 16,
    backgroundColor: '#fff',
    marginBottom: 8,
  },
  proxyInputDisabled: {
    backgroundColor: '#f5f5f5',
    color: '#999',
  },
  proxyStatus: {
    fontSize: 14,
    color: '#666',
    fontStyle: 'italic',
  },
});
