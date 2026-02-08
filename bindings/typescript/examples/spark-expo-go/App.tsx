import 'react-native-get-random-values';

import AsyncStorage from '@react-native-async-storage/async-storage';
import * as Clipboard from 'expo-clipboard';
import {
  createNode,
  installSparkRuntime,
  type InvoiceEventStatus,
  type SparkConfig,
  type SparkNode,
  type SparkRuntimeHandle,
  type Transaction,
} from '@sunnyln/lni';
import { StatusBar } from 'expo-status-bar';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  Animated,
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

type SparkDebugCheckpoint = {
  phase: string;
  ts: number;
  meta?: Record<string, unknown>;
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
  if (typeof value === 'number') return Number.isFinite(value) ? value : 0;
  if (typeof value === 'bigint') return Number(value);
  if (typeof value === 'string') { const p = Number(value); return Number.isFinite(p) ? p : 0; }
  return 0;
}

function formatSats(sats: number): string {
  return sats.toLocaleString();
}

function formatTime(unix: number): string {
  if (!unix) return '';
  const d = new Date(unix * 1000);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  if (diffMs < 60_000) return 'Just now';
  if (diffMs < 3600_000) return `${Math.floor(diffMs / 60_000)}m ago`;
  if (diffMs < 86400_000) return `${Math.floor(diffMs / 3600_000)}h ago`;
  if (d.getFullYear() === now.getFullYear()) {
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
}

type TxRow = {
  type: string;
  amountSats: number;
  memo: string;
  time: string;
  paymentHash: string;
};

function mapTx(tx: Transaction): TxRow {
  const amountSats = Math.floor(numberFromUnknown(tx.amountMsats) / 1000);
  const isIncoming = tx.type === 'incoming';
  return {
    type: tx.type,
    amountSats,
    memo: tx.description || (isIncoming ? 'Received' : 'Sent'),
    time: formatTime(tx.settledAt || tx.createdAt),
    paymentHash: tx.paymentHash,
  };
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

function TransactionRow({ tx, onPress }: { tx: TxRow; onPress?: () => void }) {
  const isIncoming = tx.type === 'incoming';
  return (
    <Pressable style={s.txRow} onPress={onPress}>
      <View style={[s.txIcon, isIncoming ? s.txIconIn : s.txIconOut]}>
        <Text style={[s.txIconText, isIncoming ? s.txIconTextIn : s.txIconTextOut]}>
          {isIncoming ? '\u2193' : '\u2191'}
        </Text>
      </View>
      <View style={s.txDetails}>
        <Text style={s.txMemo} numberOfLines={1}>{tx.memo}</Text>
        <Text style={s.txTime}>{tx.time}</Text>
      </View>
      <Text style={[s.txAmount, isIncoming ? s.txAmountIn : s.txAmountOut]}>
        {isIncoming ? '+' : '-'}{formatSats(tx.amountSats)} sats
      </Text>
    </Pressable>
  );
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export default function App() {
  // Auth / connection
  const [form, setForm] = useState<SparkFormState>(DEFAULT_FORM);
  const [connected, setConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [status, setStatus] = useState('');

  // Wallet data
  const [balanceSats, setBalanceSats] = useState(0);
  const [transactions, setTransactions] = useState<TxRow[]>([]);

  // UI state
  const [activeTab, setActiveTab] = useState<'send' | 'recv' | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [showLog, setShowLog] = useState(false);
  const [showMnemonic, setShowMnemonic] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);
  const [selectedTx, setSelectedTx] = useState<TxRow | null>(null);

  // Send
  const [sendInvoice, setSendInvoice] = useState('');
  const [sendAmountMsats, setSendAmountMsats] = useState('');
  const [sendResult, setSendResult] = useState('');
  const [sending, setSending] = useState(false);
  const [invoiceInfo, setInvoiceInfo] = useState('');

  // Receive
  const [recvAmountMsats, setRecvAmountMsats] = useState('');
  const [recvDescription, setRecvDescription] = useState('');
  const [invoiceString, setInvoiceString] = useState('');
  const [invoicePollingStatus, setInvoicePollingStatus] = useState('');

  // Debug
  const [debugJson, setDebugJson] = useState('[]');

  // Success animation
  const [showSuccess, setShowSuccess] = useState(false);
  const [successMsg, setSuccessMsg] = useState('');
  const successScale = useRef(new Animated.Value(0)).current;
  const successOpacity = useRef(new Animated.Value(0)).current;

  // Refs
  const nodeRef = useRef<SparkNode | null>(null);
  const sparkRuntimeRef = useRef<SparkRuntimeHandle | null>(null);
  const debugRef = useRef<SparkDebugCheckpoint[]>([]);
  const shouldAutoConnect = useRef(false);

  const transferLimit = useMemo(() => {
    const parsed = Number(form.transferLimit);
    return (!Number.isFinite(parsed) || parsed <= 0) ? 25 : Math.min(100, Math.floor(parsed));
  }, [form.transferLimit]);

  const updateForm = useCallback((patch: Partial<SparkFormState>) => {
    setForm((prev) => ({ ...prev, ...patch }));
  }, []);

  // Debug
  const appendDebug = useCallback((checkpoint: SparkDebugCheckpoint) => {
    const next = [...debugRef.current, checkpoint].slice(-120);
    debugRef.current = next;
    setDebugJson(JSON.stringify(next, null, 2));
  }, []);

  useEffect(() => {
    const runtime = globalThis as typeof globalThis & { __LNI_SPARK_DEBUG__?: unknown };
    runtime.__LNI_SPARK_DEBUG__ = {
      enabled: true,
      emit: (cp: SparkDebugCheckpoint) => appendDebug({ phase: cp.phase, ts: cp.ts, meta: cp.meta }),
    };
    return () => { delete runtime.__LNI_SPARK_DEBUG__; };
  }, [appendDebug]);

  // Runtime
  const setupRuntime = useCallback((apiKey: string) => {
    sparkRuntimeRef.current?.restore();
    sparkRuntimeRef.current = installSparkRuntime({ apiKey, apiKeyHeader: 'x-api-key' });
  }, []);

  const disconnectNode = useCallback(async () => {
    const node = nodeRef.current;
    nodeRef.current = null;
    if (!node) return;
    try { await node.cleanupConnections(); } catch {}
  }, []);

  useEffect(() => {
    return () => { void disconnectNode(); sparkRuntimeRef.current?.restore(); };
  }, [disconnectNode]);

  // Persistence
  const persistForm = useCallback(async (value: SparkFormState) => {
    await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(value));
  }, []);

  const loadSavedForm = useCallback(async (): Promise<boolean> => {
    const raw = await AsyncStorage.getItem(STORAGE_KEY);
    if (!raw) return false;
    const parsed = JSON.parse(raw) as Partial<SparkFormState>;
    setForm({
      ...DEFAULT_FORM,
      ...parsed,
      network: NETWORKS.includes((parsed.network as SparkNetwork) ?? 'MAINNET')
        ? (parsed.network as SparkNetwork)
        : 'MAINNET',
    });
    return Boolean(parsed.mnemonic?.trim());
  }, []);

  useEffect(() => {
    void loadSavedForm()
      .then((has) => { if (has) shouldAutoConnect.current = true; })
      .catch((e: unknown) => setStatus(e instanceof Error ? e.message : String(e)));
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

  // Refresh
  const refresh = useCallback(async () => {
    const node = nodeRef.current;
    if (!node) return;
    setStatus('Refreshing...');
    try {
      setupRuntime(form.apiKey);
      const [info, txs] = await Promise.all([
        node.getInfo(),
        node.listTransactions({ from: 0, limit: transferLimit }),
      ]);
      setBalanceSats(Math.floor(numberFromUnknown(info.sendBalanceMsat) / 1000));
      setTransactions(txs.map(mapTx));
      setStatus('');
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    }
  }, [form.apiKey, setupRuntime, transferLimit]);

  // Connect
  const connectWallet = useCallback(async () => {
    if (!form.mnemonic.trim()) { setStatus('Seed phrase is required'); return; }
    setStatus('Connecting...');
    setConnecting(true);
    try {
      await persistForm(form);
      setupRuntime(form.apiKey);
      await disconnectNode();
      const node = createNode({ kind: 'spark', config: buildSparkConfig() });
      nodeRef.current = node;
      setConnected(true);
      setStatus('Loading...');
      await refresh();
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(false);
    }
  }, [buildSparkConfig, disconnectNode, form, persistForm, refresh, setupRuntime]);

  // Auto-connect
  useEffect(() => {
    if (shouldAutoConnect.current && form.mnemonic.trim()) {
      shouldAutoConnect.current = false;
      void connectWallet();
    }
  }, [form.mnemonic, connectWallet]);

  // Decode pasted invoice
  useEffect(() => {
    const inv = sendInvoice.trim();
    if (!inv || !nodeRef.current) { setInvoiceInfo(''); return; }
    let cancelled = false;
    nodeRef.current.decode(inv).then((json) => {
      if (cancelled) return;
      try {
        const decoded = JSON.parse(json);
        const sections = Array.isArray(decoded.sections) ? decoded.sections : [];
        const amountMsats = sections.find((s: { name?: string }) => s.name === 'amount')?.value;
        const expiry = decoded.expiry ?? sections.find((s: { name?: string }) => s.name === 'expiry')?.value;
        const timestamp = sections.find((s: { name?: string }) => s.name === 'timestamp')?.value;
        const parts: string[] = [];
        if (amountMsats && Number(amountMsats) > 0) {
          parts.push(`${Math.floor(Number(amountMsats) / 1000)} sats`);
        } else {
          parts.push('No amount (open)');
        }
        if (timestamp && expiry) {
          const expiresAt = new Date((Number(timestamp) + Number(expiry)) * 1000);
          const now = new Date();
          const diffMin = Math.floor((expiresAt.getTime() - now.getTime()) / 60000);
          if (diffMin <= 0) parts.push('Expired');
          else if (diffMin < 60) parts.push(`Expires in ${diffMin}m`);
          else parts.push(`Expires in ${Math.floor(diffMin / 60)}h ${diffMin % 60}m`);
        }
        setInvoiceInfo(parts.join('  \u00b7  '));
      } catch {
        setInvoiceInfo('');
      }
    }).catch(() => { if (!cancelled) setInvoiceInfo(''); });
    return () => { cancelled = true; };
  }, [sendInvoice]);

  // Disconnect
  const disconnect = useCallback(async () => {
    await disconnectNode();
    sparkRuntimeRef.current?.restore();
    sparkRuntimeRef.current = null;
    setConnected(false);
    setBalanceSats(0);
    setTransactions([]);
    setStatus('');
    setActiveTab(null);
  }, [disconnectNode]);

  // Success animation
  const showSuccessAnimation = useCallback((label = 'Success!') => {
    setSuccessMsg(label);
    successScale.setValue(0);
    successOpacity.setValue(0);
    setShowSuccess(true);
    Animated.parallel([
      Animated.spring(successScale, { toValue: 1, friction: 5, tension: 80, useNativeDriver: true }),
      Animated.timing(successOpacity, { toValue: 1, duration: 200, useNativeDriver: true }),
    ]).start();
    setTimeout(() => {
      Animated.timing(successOpacity, { toValue: 0, duration: 300, useNativeDriver: true })
        .start(() => setShowSuccess(false));
    }, 2200);
  }, [successOpacity, successScale]);

  // Receive
  const createInvoiceReceive = useCallback(async () => {
    const node = nodeRef.current;
    if (!node) return;
    const parsed = Number(recvAmountMsats.trim());
    const amountMsats = Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) * 1000 : undefined;
    const description = recvDescription.trim() || undefined;

    setStatus('Creating invoice...');
    setInvoiceString('');
    setInvoicePollingStatus('');

    try {
      setupRuntime(form.apiKey);
      const tx = await node.createInvoice({ amountMsats, description, expiry: 3600 });
      setInvoiceString(tx.invoice);
      setInvoicePollingStatus('Waiting for payment...');
      setStatus('');

      node.onInvoiceEvents(
        { paymentHash: tx.paymentHash, pollingDelaySec: 3, maxPollingSec: 300 },
        (eventStatus: InvoiceEventStatus) => {
          if (eventStatus === 'success') {
            setInvoicePollingStatus('Paid!');
            setStatus('');
            showSuccessAnimation('Invoice Paid!');
            void refresh();
          }
        },
      ).catch((e: unknown) => console.warn('onInvoiceEvents error:', e));
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    }
  }, [form.apiKey, recvAmountMsats, recvDescription, refresh, setupRuntime, showSuccessAnimation]);

  // Send
  const sendInvoicePayment = useCallback(async () => {
    const node = nodeRef.current;
    if (!node) return;
    const invoice = sendInvoice.trim();
    if (!invoice) { setStatus('Paste an invoice first'); return; }
    const parsed = Number(sendAmountMsats.trim());
    const amountMsats = Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : undefined;

    debugRef.current = [];
    setDebugJson('[]');
    appendDebug({ phase: 'send:start', ts: Date.now(), meta: { invoiceChars: invoice.length } });

    setStatus('Sending...');
    setSendResult('');
    setSending(true);

    try {
      setupRuntime(form.apiKey);
      appendDebug({ phase: 'send:pay_call', ts: Date.now() });
      const payment = await node.payInvoice({ invoice, amountMsats });
      appendDebug({ phase: 'send:success', ts: Date.now() });
      const feeSats = Math.floor(numberFromUnknown(payment.feeMsats) / 1000);
      setSendResult(`Preimage: ${payment.preimage}\nFee: ${feeSats} sats`);
      setStatus('Sent!');
      setSendInvoice('');
      setSendAmountMsats('');
      showSuccessAnimation('Payment Sent!');
      await refresh();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      appendDebug({ phase: 'send:error', ts: Date.now(), meta: { reason: msg } });
      setSendResult(msg);
      setStatus('Send failed');
    } finally {
      setSending(false);
    }
  }, [appendDebug, form.apiKey, refresh, sendAmountMsats, sendInvoice, setupRuntime]);

  const copyInvoice = useCallback(async () => {
    if (!invoiceString) return;
    try { await Clipboard.setStringAsync(invoiceString); setStatus('Copied!'); } catch {}
  }, [invoiceString]);

  const copyDebug = useCallback(async () => {
    try { await Clipboard.setStringAsync(debugJson); setStatus('Log copied'); } catch {}
  }, [debugJson]);

  // ---------- RENDER ----------

  if (!connected) {
    return (
      <SafeAreaView style={s.root}>
        <StatusBar style="light" />
        <ScrollView contentContainerStyle={s.loginContainer} keyboardShouldPersistTaps="handled">
          <Text style={s.logo}>Spark Wallet</Text>
          <Text style={s.logoSub}>Pure TypeScript &middot; No WASM</Text>

          <Text style={s.fieldLabel}>Seed Phrase</Text>
          <View style={s.secretField}>
            <TextInput
              value={form.mnemonic}
              onChangeText={(v) => updateForm({ mnemonic: v })}
              multiline={showMnemonic} secureTextEntry={!showMnemonic}
              autoCapitalize="none" autoCorrect={false}
              placeholder="Enter your 12 or 24 word mnemonic..."
              placeholderTextColor="#555" style={[s.input, s.secretInput, !showMnemonic && { minHeight: undefined }]}
            />
            <Pressable style={s.eyeBtn} onPress={() => setShowMnemonic((v) => !v)} hitSlop={8}>
              <Text style={s.eyeIcon}>{showMnemonic ? '\u{1F441}' : '\u{2022}\u{2022}\u{2022}'}</Text>
            </Pressable>
          </View>

          <Text style={s.fieldLabel}>Network</Text>
          <View style={s.networkRow}>
            {NETWORKS.map((n) => (
              <Pressable key={n} style={[s.chip, form.network === n && s.chipActive]} onPress={() => updateForm({ network: n })}>
                <Text style={[s.chipText, form.network === n && s.chipTextActive]}>{n}</Text>
              </Pressable>
            ))}
          </View>

          <Text style={s.fieldLabel}>API Key <Text style={s.optionalTag}>(optional)</Text></Text>
          <View style={s.secretField}>
            <TextInput
              value={form.apiKey} onChangeText={(v) => updateForm({ apiKey: v })}
              secureTextEntry={!showApiKey} autoCapitalize="none" autoCorrect={false}
              placeholder="Optional" placeholderTextColor="#555" style={[s.input, s.secretInput]}
            />
            <Pressable style={s.eyeBtn} onPress={() => setShowApiKey((v) => !v)} hitSlop={8}>
              <Text style={s.eyeIcon}>{showApiKey ? '\u{1F441}' : '\u{2022}\u{2022}\u{2022}'}</Text>
            </Pressable>
          </View>

          <Pressable
            style={[s.btn, s.btnOrange, connecting && s.btnDisabled]}
            onPress={() => void connectWallet()}
            disabled={connecting}
          >
            {connecting ? (
              <View style={s.btnRow}><ActivityIndicator size="small" color="#000" /><Text style={s.btnTextDark}>Connecting...</Text></View>
            ) : (
              <Text style={s.btnTextDark}>Connect Wallet</Text>
            )}
          </Pressable>

          {status !== '' && <Text style={s.statusText}>{status}</Text>}
        </ScrollView>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={s.root}>
      <StatusBar style="light" />
      <ScrollView contentContainerStyle={s.walletContainer} keyboardShouldPersistTaps="handled">
        {/* Header */}
        <View style={s.header}>
          <Text style={s.headerTitle}>Spark Wallet</Text>
          <View style={s.headerActions}>
            <Pressable onPress={() => void refresh()} hitSlop={8}><Text style={s.headerIcon}>&#x21bb;</Text></Pressable>
            <Pressable onPress={() => setShowSettings((v) => !v)} hitSlop={8}><Text style={s.headerIcon}>&#x2699;</Text></Pressable>
            <Pressable onPress={() => void disconnect()} hitSlop={8}><Text style={s.headerIcon}>&#x23fb;</Text></Pressable>
          </View>
        </View>

        {/* Balance */}
        <View style={s.balanceCard}>
          <Text style={s.balanceLabel}>BALANCE</Text>
          <Text style={s.balanceValue}>{formatSats(balanceSats)} <Text style={s.balanceUnit}>sats</Text></Text>
          {status !== '' && <Text style={s.balanceStatus}>{status}</Text>}
        </View>

        {/* Send / Receive buttons */}
        <View style={s.tabBar}>
          <Pressable style={[s.tabBtn, s.tabSend, activeTab === 'send' && s.tabActive]} onPress={() => setActiveTab(activeTab === 'send' ? null : 'send')}>
            <Text style={s.tabSendText}>{'\u2191'} Send</Text>
          </Pressable>
          <Pressable style={[s.tabBtn, s.tabRecv, activeTab === 'recv' && s.tabActive]} onPress={() => setActiveTab(activeTab === 'recv' ? null : 'recv')}>
            <Text style={s.tabRecvText}>{'\u2193'} Receive</Text>
          </Pressable>
        </View>

        {/* Send panel */}
        {activeTab === 'send' && (
          <View style={s.panel}>
            <Text style={s.fieldLabel}>Invoice</Text>
            <TextInput
              value={sendInvoice} onChangeText={setSendInvoice}
              multiline autoCapitalize="none" autoCorrect={false}
              placeholder="Paste a Lightning invoice (lnbc...)"
              placeholderTextColor="#555" style={[s.input, s.textarea]}
            />
            {invoiceInfo !== '' && <Text style={s.invoiceInfo}>{invoiceInfo}</Text>}
            <Text style={s.fieldLabel}>Amount (msats, optional)</Text>
            <TextInput
              value={sendAmountMsats} onChangeText={setSendAmountMsats}
              keyboardType="number-pad" placeholder="For zero-amount invoices"
              placeholderTextColor="#555" style={s.input}
            />
            <Pressable
              style={[s.btn, s.btnOrange, sending && s.btnDisabled]}
              onPress={() => void sendInvoicePayment()}
              disabled={sending}
            >
              {sending ? (
                <View style={s.btnRow}><ActivityIndicator size="small" color="#000" /><Text style={s.btnTextDark}>Sending...</Text></View>
              ) : (
                <Text style={s.btnTextDark}>Send Payment</Text>
              )}
            </Pressable>
            {sendResult !== '' && <Text style={s.resultText}>{sendResult}</Text>}
          </View>
        )}

        {/* Receive panel */}
        {activeTab === 'recv' && (
          <View style={s.panel}>
            <Text style={s.fieldLabel}>Amount (sats)</Text>
            <TextInput
              value={recvAmountMsats} onChangeText={setRecvAmountMsats}
              keyboardType="number-pad" placeholder="e.g. 25"
              placeholderTextColor="#555" style={s.input}
            />
            <Text style={s.fieldLabel}>Description</Text>
            <TextInput
              value={recvDescription} onChangeText={setRecvDescription}
              autoCapitalize="none" autoCorrect={false}
              placeholder="Optional memo" placeholderTextColor="#555" style={s.input}
            />
            <Pressable style={[s.btn, s.btnOrange]} onPress={() => void createInvoiceReceive()}>
              <Text style={s.btnTextDark}>Create Invoice</Text>
            </Pressable>

            {invoicePollingStatus !== '' && (
              <Text style={[
                s.pollingText,
                invoicePollingStatus === 'Paid!' && { color: '#4ade80' },
                invoicePollingStatus === 'Failed' && { color: '#f87171' },
                invoicePollingStatus === 'Timed out' && { color: '#fbbf24' },
              ]}>{invoicePollingStatus}</Text>
            )}

            {invoiceString !== '' && (
              <Pressable onPress={() => void copyInvoice()}>
                <Text style={s.resultText}>{invoiceString}</Text>
                <Text style={s.copyHint}>Tap to copy</Text>
              </Pressable>
            )}
          </View>
        )}

        {/* Settings */}
        {showSettings && (
          <View style={s.settingsPanel}>
            <Text style={s.fieldLabel}>Seed Phrase</Text>
            <View style={s.secretField}>
              <TextInput
                value={form.mnemonic}
                onChangeText={(v) => updateForm({ mnemonic: v })}
                multiline={showMnemonic} secureTextEntry={!showMnemonic}
                autoCapitalize="none" autoCorrect={false}
                placeholder="12 or 24 word mnemonic..."
                placeholderTextColor="#555" style={[s.input, s.secretInput, !showMnemonic && { minHeight: undefined }]}
              />
              <Pressable style={s.eyeBtn} onPress={() => setShowMnemonic((v) => !v)} hitSlop={8}>
                <Text style={s.eyeIcon}>{showMnemonic ? '\u{1F441}' : '\u{2022}\u{2022}\u{2022}'}</Text>
              </Pressable>
            </View>
            <Text style={s.fieldLabel}>API Key <Text style={s.optionalTag}>(optional)</Text></Text>
            <View style={s.secretField}>
              <TextInput
                value={form.apiKey} onChangeText={(v) => updateForm({ apiKey: v })}
                secureTextEntry={!showApiKey} autoCapitalize="none" autoCorrect={false}
                placeholder="Optional" placeholderTextColor="#555" style={[s.input, s.secretInput]}
              />
              <Pressable style={s.eyeBtn} onPress={() => setShowApiKey((v) => !v)} hitSlop={8}>
                <Text style={s.eyeIcon}>{showApiKey ? '\u{1F441}' : '\u{2022}\u{2022}\u{2022}'}</Text>
              </Pressable>
            </View>
            <Text style={s.fieldLabel}>SSP Base URL <Text style={s.optionalTag}>(optional)</Text></Text>
            <TextInput
              value={form.sspBaseUrl} onChangeText={(v) => updateForm({ sspBaseUrl: v })}
              autoCapitalize="none" autoCorrect={false}
              placeholder="https://..." placeholderTextColor="#555" style={s.input}
            />
            <Text style={s.fieldLabel}>SSP Identity Public Key <Text style={s.optionalTag}>(optional)</Text></Text>
            <TextInput
              value={form.sspIdentityPublicKey} onChangeText={(v) => updateForm({ sspIdentityPublicKey: v })}
              autoCapitalize="none" autoCorrect={false}
              placeholder="Hex pubkey" placeholderTextColor="#555" style={s.input}
            />
            <Text style={s.fieldLabel}>Transaction Limit <Text style={s.optionalTag}>(optional)</Text></Text>
            <TextInput
              value={form.transferLimit} onChangeText={(v) => updateForm({ transferLimit: v })}
              keyboardType="number-pad" placeholder="25"
              placeholderTextColor="#555" style={s.input}
            />
            <Pressable style={[s.btn, s.btnGhost]} onPress={() => { void persistForm(form); setStatus('Settings saved'); setShowSettings(false); }}>
              <Text style={s.btnTextLight}>Save Settings</Text>
            </Pressable>
          </View>
        )}

        {/* Transactions */}
        <View style={s.txSection}>
          <Text style={s.txTitle}>Transactions</Text>
          {transactions.length === 0 ? (
            <Text style={s.txEmpty}>No transactions yet</Text>
          ) : (
            transactions.map((tx, i) => <TransactionRow key={`${i}-${tx.paymentHash}`} tx={tx} onPress={() => setSelectedTx(tx)} />)
          )}
        </View>

        {/* Log toggle */}
        <Pressable style={[s.btn, s.btnGhost, { marginHorizontal: 20, marginTop: 8 }]} onPress={() => setShowLog((v) => !v)}>
          <Text style={s.btnTextLight}>{showLog ? 'Hide Log' : 'Show Log'}</Text>
        </Pressable>

        {showLog && (
          <View style={s.logSection}>
            <Pressable style={[s.btn, s.btnGhost]} onPress={() => void copyDebug()}>
              <Text style={s.btnTextLight}>Copy Log</Text>
            </Pressable>
            <Text selectable style={s.logText}>{debugJson}</Text>
          </View>
        )}
      </ScrollView>

      {/* Transaction detail modal */}
      {selectedTx && (
        <Pressable style={s.txDetailOverlay} onPress={() => setSelectedTx(null)}>
          <View style={s.txDetailCard}>
            <View style={[s.txDetailIcon, selectedTx.type === 'incoming' ? s.txIconIn : s.txIconOut]}>
              <Text style={[s.txDetailIconText, selectedTx.type === 'incoming' ? s.txIconTextIn : s.txIconTextOut]}>
                {selectedTx.type === 'incoming' ? '\u2193' : '\u2191'}
              </Text>
            </View>
            <Text style={[s.txDetailAmount, selectedTx.type === 'incoming' ? s.txAmountIn : s.txAmountOut]}>
              {selectedTx.type === 'incoming' ? '+' : '-'}{formatSats(selectedTx.amountSats)} sats
            </Text>
            <Text style={s.txDetailType}>{selectedTx.type === 'incoming' ? 'Received' : 'Sent'}</Text>
            <View style={s.txDetailRows}>
              <View style={s.txDetailRow}><Text style={s.txDetailLabel}>Memo</Text><Text style={s.txDetailValue}>{selectedTx.memo}</Text></View>
              <View style={s.txDetailRow}><Text style={s.txDetailLabel}>Time</Text><Text style={s.txDetailValue}>{selectedTx.time}</Text></View>
              {selectedTx.paymentHash ? (
                <View style={s.txDetailRow}><Text style={s.txDetailLabel}>Payment Hash</Text><Text style={s.txDetailHash} selectable>{selectedTx.paymentHash}</Text></View>
              ) : null}
            </View>
            <Pressable style={[s.btn, s.btnGhost, { marginTop: 16 }]} onPress={() => setSelectedTx(null)}>
              <Text style={s.btnTextLight}>Close</Text>
            </Pressable>
          </View>
        </Pressable>
      )}

      {/* Success overlay */}
      {showSuccess && (
        <Pressable style={s.successOverlay} onPress={() => setShowSuccess(false)}>
          <Animated.View style={[s.successCircle, { transform: [{ scale: successScale }], opacity: successOpacity }]}>
            <Text style={s.successCheck}>{'\u2713'}</Text>
          </Animated.View>
          <Animated.Text style={[s.successLabel, { opacity: successOpacity }]}>{successMsg}</Animated.Text>
        </Pressable>
      )}
    </SafeAreaView>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const ORANGE = '#f7931a';
const ORANGE_DIM = 'rgba(247, 147, 26, 0.15)';

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: '#0a0e1a' },

  // Login
  loginContainer: { flexGrow: 1, justifyContent: 'center', padding: 24, gap: 14 },
  logo: { textAlign: 'center', fontSize: 28, fontWeight: '800', color: ORANGE },
  logoSub: { textAlign: 'center', color: '#64748b', fontSize: 13, marginBottom: 8 },

  // Fields
  fieldLabel: { color: '#64748b', fontSize: 11, fontWeight: '700', textTransform: 'uppercase', letterSpacing: 0.5, marginTop: 6 },
  optionalTag: { color: '#475569', fontWeight: '400', textTransform: 'none', letterSpacing: 0 },
  input: { borderWidth: 1, borderColor: '#1e293b', borderRadius: 12, paddingHorizontal: 14, paddingVertical: 12, color: '#f1f5f9', backgroundColor: '#0f172a', fontSize: 15 },
  secretField: { position: 'relative' as const },
  secretInput: { paddingRight: 48 },
  eyeBtn: { position: 'absolute' as const, right: 12, top: 0, bottom: 0, justifyContent: 'center' as const },
  eyeIcon: { fontSize: 18, color: '#64748b' },
  textarea: { minHeight: 80, textAlignVertical: 'top' },

  // Networks
  networkRow: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginTop: 4 },
  chip: { borderWidth: 1, borderColor: '#1e293b', borderRadius: 999, paddingHorizontal: 14, paddingVertical: 7, backgroundColor: '#0f172a' },
  chipActive: { backgroundColor: ORANGE, borderColor: ORANGE },
  chipText: { color: '#94a3b8', fontSize: 11, fontWeight: '700' },
  chipTextActive: { color: '#000' },

  // Buttons
  btn: { borderRadius: 12, paddingVertical: 14, alignItems: 'center', justifyContent: 'center' },
  btnOrange: { backgroundColor: ORANGE },
  btnGhost: { backgroundColor: 'transparent', borderWidth: 1, borderColor: '#1e293b' },
  btnDisabled: { opacity: 0.4 },
  btnTextDark: { color: '#000', fontWeight: '700', fontSize: 15 },
  btnTextLight: { color: '#94a3b8', fontWeight: '600', fontSize: 14 },
  btnRow: { flexDirection: 'row', alignItems: 'center', gap: 8 },

  statusText: { color: '#94a3b8', fontSize: 13, textAlign: 'center', marginTop: 4 },

  // Wallet
  walletContainer: { paddingBottom: 40 },
  header: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', paddingHorizontal: 20, paddingTop: 12, paddingBottom: 4 },
  headerTitle: { color: '#64748b', fontWeight: '700', fontSize: 15 },
  headerActions: { flexDirection: 'row', gap: 16 },
  headerIcon: { color: '#64748b', fontSize: 18 },

  // Balance
  balanceCard: { alignItems: 'center', paddingVertical: 32 },
  balanceLabel: { color: '#64748b', fontSize: 11, fontWeight: '700', letterSpacing: 1 },
  balanceValue: { color: '#f1f5f9', fontSize: 42, fontWeight: '800', marginTop: 8, letterSpacing: -1 },
  balanceUnit: { fontSize: 18, fontWeight: '600', color: '#64748b' },
  balanceStatus: { color: '#64748b', fontSize: 12, marginTop: 6 },

  // Tabs
  tabBar: { flexDirection: 'row', gap: 12, paddingHorizontal: 20, marginBottom: 16 },
  tabBtn: { flex: 1, borderRadius: 14, paddingVertical: 14, alignItems: 'center' },
  tabSend: { backgroundColor: ORANGE },
  tabRecv: { backgroundColor: ORANGE_DIM },
  tabActive: { opacity: 1 },
  tabSendText: { color: '#000', fontWeight: '700', fontSize: 15 },
  tabRecvText: { color: ORANGE, fontWeight: '700', fontSize: 15 },

  // Panel
  panel: { paddingHorizontal: 20, paddingBottom: 16, gap: 10 },
  invoiceInfo: { color: '#60a5fa', fontSize: 13, fontWeight: '600' },
  resultText: { backgroundColor: '#0f172a', borderWidth: 1, borderColor: '#1e293b', borderRadius: 10, padding: 12, fontFamily: 'Courier', fontSize: 12, color: '#94a3b8', lineHeight: 16 },
  pollingText: { color: '#60a5fa', fontSize: 13, fontWeight: '600' },
  copyHint: { color: '#475569', fontSize: 11, textAlign: 'center', marginTop: 4 },

  // Settings
  settingsPanel: { paddingHorizontal: 20, paddingVertical: 16, gap: 10, borderTopWidth: 1, borderTopColor: '#1e293b' },

  // Transactions
  txSection: { paddingHorizontal: 20, paddingTop: 8 },
  txTitle: { color: '#f1f5f9', fontSize: 15, fontWeight: '700', marginBottom: 12 },
  txEmpty: { color: '#475569', fontSize: 14, textAlign: 'center', paddingVertical: 40 },
  txRow: { flexDirection: 'row', alignItems: 'center', gap: 12, paddingVertical: 12 },
  txIcon: { width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center' },
  txIconIn: { backgroundColor: 'rgba(74, 222, 128, 0.12)' },
  txIconOut: { backgroundColor: 'rgba(248, 113, 113, 0.12)' },
  txIconText: { fontSize: 16, fontWeight: '700' },
  txIconTextIn: { color: '#4ade80' },
  txIconTextOut: { color: '#f87171' },
  txDetails: { flex: 1 },
  txMemo: { color: '#f1f5f9', fontSize: 14, fontWeight: '500' },
  txTime: { color: '#475569', fontSize: 12, marginTop: 2 },
  txAmount: { fontSize: 14, fontWeight: '600' },
  txAmountIn: { color: '#4ade80' },
  txAmountOut: { color: '#f1f5f9' },

  // Log
  logSection: { paddingHorizontal: 20, paddingTop: 10, gap: 8 },
  logText: { backgroundColor: '#020617', borderWidth: 1, borderColor: '#1e293b', borderRadius: 10, padding: 12, color: '#475569', fontFamily: 'Courier', fontSize: 10, lineHeight: 14 },

  // Transaction detail modal
  txDetailOverlay: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(0,0,0,0.6)', alignItems: 'center', justifyContent: 'center', zIndex: 999 },
  txDetailCard: { backgroundColor: '#111827', borderRadius: 20, padding: 24, width: '85%', alignItems: 'center', borderWidth: 1, borderColor: '#1e293b' },
  txDetailIcon: { width: 52, height: 52, borderRadius: 26, alignItems: 'center', justifyContent: 'center', marginBottom: 12 },
  txDetailIconText: { fontSize: 24, fontWeight: '700' },
  txDetailAmount: { fontSize: 28, fontWeight: '800', marginBottom: 4 },
  txDetailType: { color: '#64748b', fontSize: 13, fontWeight: '600', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 20 },
  txDetailRows: { width: '100%', gap: 12 },
  txDetailRow: { borderBottomWidth: 1, borderBottomColor: '#1e293b', paddingBottom: 10 },
  txDetailLabel: { color: '#64748b', fontSize: 11, fontWeight: '600', textTransform: 'uppercase', marginBottom: 4 },
  txDetailValue: { color: '#f1f5f9', fontSize: 14 },
  txDetailHash: { color: '#94a3b8', fontSize: 11, fontFamily: 'Courier' },

  // Success
  successOverlay: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(0,0,0,0.6)', alignItems: 'center', justifyContent: 'center', zIndex: 1000 },
  successCircle: { width: 100, height: 100, borderRadius: 50, backgroundColor: '#4ade80', alignItems: 'center', justifyContent: 'center', shadowColor: '#4ade80', shadowOffset: { width: 0, height: 0 }, shadowOpacity: 0.5, shadowRadius: 30, elevation: 20 },
  successCheck: { color: '#fff', fontSize: 48, fontWeight: '700' },
  successLabel: { color: '#fff', fontSize: 18, fontWeight: '700', marginTop: 16 },
});
