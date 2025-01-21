import { useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { PhoenixdNode } from '../../src/generated/lni_sdk-ffi';

export default function App() {
  const [offer, setOffer] = useState<string>('');

  const main = async () => {
    try {
      const result = new PhoenixdNode().uniffiUse((obj) => {
        obj.callSomeMethod();
        return obj.callAnotherMethod();
      });

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
