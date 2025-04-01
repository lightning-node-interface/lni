yarn ubrn:clean
rm -rf rust_modules
yarn ubrn:checkout
yarn ubrn:android
# yarn ubrn:ios
# cd example/ios && pod install --repo-update && cd ../     <-- run this if you get errors to updates the pods
# cd example && yarn ios