import { useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { PhoenixdNode, Fetcher } from 'react-native-lni';

export default function App() {
  const [offer, setOffer] = useState<string>('');

  const main = async () => {
    try {
      const node =  new PhoenixdNode();



      //const offerRes = await node.getOffer();
      //setOffer(offerRes);
    } catch (e) {
      console.error(e);
    }
  };

  main();

  return (
    <View style={styles.container}>
      <Text>Offer: {offer}</Text>
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
