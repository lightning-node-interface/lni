import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { Fetcher } from 'react-native-lni';

export default function App() {
  const fetcher = new Fetcher('http://woot.com');
  const [ip, setIp] = useState<string>('');

  const main = async () => {
    const ipAddr = await fetcher.getIpAddress();
    setIp(ipAddr.origin);
    console.log('IP Address', ipAddr);
  };

  useEffect(() => {
    main();
  }, []);

  return (
    <View style={styles.container}>
      <Text>Your IP address is shit: {ip}</Text>
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
