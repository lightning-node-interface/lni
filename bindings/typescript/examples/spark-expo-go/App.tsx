import 'react-native-get-random-values';

import AsyncStorage from '@react-native-async-storage/async-storage';
import {
  createNode,
  installSparkRuntime,
  type SparkConfig,
  type SparkNode,
  type SparkRuntimeHandle,
  type Transaction,
} from '@sunnyln/lni';
import { StatusBar } from 'expo-status-bar';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Pressable,
  SafeAreaView,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

type SparkNetwork = 'MAINNET' | 'TESTNET' | 'REGTEST' | 'SIGNET' | 'LOCAL';

type SparkFormState = {
  mnemonic: string;
  network: SparkNetwork;
  apiKey: string;
  sspBaseUrl: string;
  sspIdentityPublicKey: string;
  transferLimit: string;
};

type TransferPreview = {
  id: string;
  direction: string;
  status: string;
  totalValue: string;
  createdTime: string;
  paymentHash: string;
  memo: string;
};

const STORAGE_KEY = 'lni.spark.expo-go.v1';
const NETWORKS: SparkNetwork[] = ['MAINNET', 'TESTNET', 'REGTEST', 'SIGNET', 'LOCAL'];
const NETWORK_TO_SPARK_CONFIG: Record<SparkNetwork, SparkConfig['network']> = {
  MAINNET: 'mainnet',
  TESTNET: 'testnet',
  REGTEST: 'regtest',
  SIGNET: 'signet',
  LOCAL: 'local',
};

const DEFAULT_FORM: SparkFormState = {
  mnemonic: '',
  network: 'MAINNET',
  apiKey: '',
  sspBaseUrl: '',
  sspIdentityPublicKey: '',
  transferLimit: '25',
};

function numberFromUnknown(value: unknown): number {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : 0;
  }
  if (typeof value === 'bigint') {
    return Number(value);
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function mapTransaction(tx: Transaction): TransferPreview {
  const amountSats = Math.floor(numberFromUnknown(tx.amountMsats) / 1000);
  return {
    id: tx.externalId || tx.paymentHash || tx.invoice || '',
    direction: tx.type,
    status: tx.settledAt > 0 ? 'SETTLED' : 'PENDING',
    totalValue: String(amountSats),
    createdTime: tx.createdAt > 0 ? new Date(tx.createdAt * 1000).toISOString() : '',
    paymentHash: tx.paymentHash,
    memo: tx.description,
  };
}

export default function App() {
  const [form, setForm] = useState<SparkFormState>(DEFAULT_FORM);
  const [status, setStatus] = useState('idle');
  const [summaryJson, setSummaryJson] = useState('{}');
  const [transactionsJson, setTransactionsJson] = useState('[]');

  const nodeRef = useRef<SparkNode | null>(null);
  const sparkRuntimeRef = useRef<SparkRuntimeHandle | null>(null);

  const transferLimit = useMemo(() => {
    const parsed = Number(form.transferLimit);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      return 25;
    }
    return Math.min(100, Math.floor(parsed));
  }, [form.transferLimit]);

  const updateForm = useCallback((patch: Partial<SparkFormState>) => {
    setForm((previous) => ({ ...previous, ...patch }));
  }, []);

  const setupRuntime = useCallback((apiKey: string) => {
    sparkRuntimeRef.current?.restore();
    sparkRuntimeRef.current = installSparkRuntime({
      apiKey,
      apiKeyHeader: 'x-api-key',
    });
  }, []);

  const disconnectNode = useCallback(async () => {
    const node = nodeRef.current;
    nodeRef.current = null;
    if (!node) {
      return;
    }

    try {
      await node.cleanupConnections();
    } catch {
      // ignore teardown failures in demo app
    }
  }, []);

  useEffect(() => {
    return () => {
      void disconnectNode();
      sparkRuntimeRef.current?.restore();
      sparkRuntimeRef.current = null;
    };
  }, [disconnectNode]);

  const persistForm = useCallback(async (value: SparkFormState) => {
    await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(value));
  }, []);

  const loadSavedForm = useCallback(async () => {
    const raw = await AsyncStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return;
    }

    const parsed = JSON.parse(raw) as Partial<SparkFormState>;
    setForm({
      ...DEFAULT_FORM,
      ...parsed,
      network: NETWORKS.includes((parsed.network as SparkNetwork) ?? 'MAINNET')
        ? (parsed.network as SparkNetwork)
        : 'MAINNET',
    });
  }, []);

  useEffect(() => {
    void loadSavedForm().catch((error: unknown) => {
      setStatus(`failed to load saved config: ${error instanceof Error ? error.message : String(error)}`);
    });
  }, [loadSavedForm]);

  const buildSparkConfig = useCallback((): SparkConfig => {
    const sparkOptions: Record<string, unknown> = {};

    if (form.sspBaseUrl.trim() && form.sspIdentityPublicKey.trim()) {
      sparkOptions.sspClientOptions = {
        baseUrl: form.sspBaseUrl.trim(),
        identityPublicKey: form.sspIdentityPublicKey.trim(),
      };
    }

    return {
      mnemonic: form.mnemonic.trim(),
      network: NETWORK_TO_SPARK_CONFIG[form.network],
      sdkEntry: 'bare',
      sparkOptions: Object.keys(sparkOptions).length ? sparkOptions : undefined,
    };
  }, [form.mnemonic, form.network, form.sspBaseUrl, form.sspIdentityPublicKey]);

  const refresh = useCallback(async () => {
    const node = nodeRef.current;
    if (!node) {
      setStatus('connect wallet first');
      return;
    }

    setStatus('refreshing balance + transfers...');

    try {
      setupRuntime(form.apiKey);

      const [info, transactions] = await Promise.all([
        node.getInfo(),
        node.listTransactions({ from: 0, limit: transferLimit }),
      ]);

      const mapped = transactions.map(mapTransaction);

      setSummaryJson(
        JSON.stringify(
          {
            network: info.network,
            identityPublicKey: info.pubkey,
            balanceSats: Math.floor(numberFromUnknown(info.sendBalanceMsat) / 1000),
            transferCount: mapped.length,
          },
          null,
          2,
        ),
      );

      setTransactionsJson(JSON.stringify(mapped, null, 2));
      setStatus('loaded');
    } catch (error) {
      setStatus(`refresh failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }, [form.apiKey, setupRuntime, transferLimit]);

  const connectWallet = useCallback(async () => {
    if (!form.mnemonic.trim()) {
      setStatus('mnemonic is required');
      return;
    }

    setStatus('connecting wallet...');

    try {
      await persistForm(form);
      setupRuntime(form.apiKey);
      await disconnectNode();
      const node = createNode({
        kind: 'spark',
        config: buildSparkConfig(),
      });

      nodeRef.current = node;
      setStatus('wallet connected');
      await refresh();
    } catch (error) {
      setStatus(`connect failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }, [buildSparkConfig, disconnectNode, form, persistForm, refresh, setupRuntime]);

  const clearSavedData = useCallback(async () => {
    await AsyncStorage.removeItem(STORAGE_KEY);
    await disconnectNode();
    setForm(DEFAULT_FORM);
    setSummaryJson('{}');
    setTransactionsJson('[]');
    sparkRuntimeRef.current?.restore();
    sparkRuntimeRef.current = null;
    setStatus('cleared saved data');
  }, [disconnectNode]);

  return (
    <SafeAreaView style={styles.root}>
      <StatusBar style="light" />
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.title}>Spark Expo Go Demo (No WASM)</Text>
        <Text style={styles.subtitle}>
          Uses createNode for Spark + installSparkRuntime() with AsyncStorage.
        </Text>

        <Text style={styles.label}>Mnemonic</Text>
        <TextInput
          value={form.mnemonic}
          onChangeText={(value) => updateForm({ mnemonic: value })}
          multiline
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="abandon ..."
          placeholderTextColor="#7d8590"
          style={[styles.input, styles.textarea]}
        />

        <Text style={styles.label}>Network</Text>
        <View style={styles.networkRow}>
          {NETWORKS.map((network) => {
            const active = form.network === network;
            return (
              <Pressable
                key={network}
                style={[styles.networkChip, active ? styles.networkChipActive : null]}
                onPress={() => updateForm({ network })}
              >
                <Text style={[styles.networkChipText, active ? styles.networkChipTextActive : null]}>
                  {network}
                </Text>
              </Pressable>
            );
          })}
        </View>

        <Text style={styles.label}>API Key (optional)</Text>
        <TextInput
          value={form.apiKey}
          onChangeText={(value) => updateForm({ apiKey: value })}
          autoCapitalize="none"
          autoCorrect={false}
          secureTextEntry
          placeholder="optional"
          placeholderTextColor="#7d8590"
          style={styles.input}
        />

        <Text style={styles.label}>SSP Base URL (optional)</Text>
        <TextInput
          value={form.sspBaseUrl}
          onChangeText={(value) => updateForm({ sspBaseUrl: value })}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="https://..."
          placeholderTextColor="#7d8590"
          style={styles.input}
        />

        <Text style={styles.label}>SSP Identity Public Key (optional)</Text>
        <TextInput
          value={form.sspIdentityPublicKey}
          onChangeText={(value) => updateForm({ sspIdentityPublicKey: value })}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="hex pubkey"
          placeholderTextColor="#7d8590"
          style={styles.input}
        />

        <Text style={styles.label}>Transfer Limit</Text>
        <TextInput
          value={form.transferLimit}
          onChangeText={(value) => updateForm({ transferLimit: value })}
          keyboardType="number-pad"
          placeholder="25"
          placeholderTextColor="#7d8590"
          style={styles.input}
        />

        <View style={styles.buttonRow}>
          <Pressable style={[styles.button, styles.primaryButton]} onPress={() => void connectWallet()}>
            <Text style={styles.buttonText}>Connect Wallet</Text>
          </Pressable>
          <Pressable style={styles.button} onPress={() => void refresh()}>
            <Text style={styles.buttonText}>Refresh</Text>
          </Pressable>
          <Pressable style={[styles.button, styles.dangerButton]} onPress={() => void clearSavedData()}>
            <Text style={styles.buttonText}>Clear</Text>
          </Pressable>
        </View>

        <Text style={styles.status}>Status: {status}</Text>

        <Text style={styles.sectionTitle}>Wallet Summary</Text>
        <Text selectable style={styles.jsonBlock}>
          {summaryJson}
        </Text>

        <Text style={styles.sectionTitle}>Transactions</Text>
        <Text selectable style={styles.jsonBlock}>
          {transactionsJson}
        </Text>
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: '#05070f',
  },
  content: {
    padding: 16,
    paddingBottom: 40,
  },
  title: {
    color: '#f3f4f6',
    fontSize: 22,
    fontWeight: '700',
  },
  subtitle: {
    color: '#8f9aaa',
    marginTop: 6,
    marginBottom: 16,
  },
  label: {
    color: '#d1d5db',
    marginBottom: 6,
    marginTop: 10,
    fontWeight: '600',
  },
  input: {
    borderWidth: 1,
    borderColor: '#364152',
    borderRadius: 10,
    paddingHorizontal: 12,
    paddingVertical: 10,
    color: '#e5e7eb',
    backgroundColor: '#0f1729',
  },
  textarea: {
    minHeight: 100,
    textAlignVertical: 'top',
  },
  networkRow: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 8,
  },
  networkChip: {
    borderWidth: 1,
    borderColor: '#364152',
    borderRadius: 999,
    paddingHorizontal: 12,
    paddingVertical: 7,
    backgroundColor: '#0f1729',
  },
  networkChipActive: {
    backgroundColor: '#1f4fd6',
    borderColor: '#1f4fd6',
  },
  networkChipText: {
    color: '#d1d5db',
    fontSize: 12,
    fontWeight: '600',
  },
  networkChipTextActive: {
    color: '#ffffff',
  },
  buttonRow: {
    marginTop: 16,
    gap: 10,
  },
  button: {
    borderRadius: 10,
    borderWidth: 1,
    borderColor: '#344256',
    backgroundColor: '#111827',
    paddingVertical: 12,
    alignItems: 'center',
  },
  primaryButton: {
    backgroundColor: '#2563eb',
    borderColor: '#2563eb',
  },
  dangerButton: {
    backgroundColor: '#7f1d1d',
    borderColor: '#7f1d1d',
  },
  buttonText: {
    color: '#f9fafb',
    fontWeight: '700',
  },
  status: {
    marginTop: 14,
    color: '#8ab4ff',
    fontSize: 13,
  },
  sectionTitle: {
    marginTop: 16,
    marginBottom: 6,
    color: '#e5e7eb',
    fontSize: 16,
    fontWeight: '700',
  },
  jsonBlock: {
    borderWidth: 1,
    borderColor: '#364152',
    borderRadius: 10,
    padding: 12,
    backgroundColor: '#020617',
    color: '#e2e8f0',
    fontFamily: 'Courier',
    fontSize: 12,
    lineHeight: 16,
  },
});
