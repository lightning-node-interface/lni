yarn ubrn:clean
rm -rf rust_modules
rm -rf lib
yarn ubrn:checkout

#### android
yarn ubrn:android
# yarn prepare
# cd example && yarn clean:android && yarn && yarn build:android && yarn android
# troubleshooting yarn react-native doctor 
# yarn package

#### ios
yarn ubrn:ios
# cd example/ios && pod install --repo-update && cd ../     <-- run this if you get errors to updates the pods
# cd example && yarn ios