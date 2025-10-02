# Deep clean
rm -rf node_modules
rm -rf example/node_modules
rm -rf lib
rm -rf android/build
rm -rf example/android/build
rm -rf example/android/app/build
rm -rf example/ios/build
rm -rf example/ios/Pods
rm -rf .yarn/cache

# Clear caches
yarn cache clean --all

# Reinstall
yarn
cd example && yarn