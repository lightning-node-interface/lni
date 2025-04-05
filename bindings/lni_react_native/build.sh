yarn ubrn:clean
rm -rf rust_modules
yarn ubrn:checkout

#### android
yarn ubrn:android
# cd example && yarn build:android && yarn android

#### ios
# yarn ubrn:ios
# cd example/ios && pod install --repo-update && cd ../     <-- run this if you get errors to updates the pods
# cd example && yarn ios