# Upgrade swiftformat to latest
brew install swiftformat

swiftformat . --lint

./build-ios.sh --release

brew install xcbeautify

# test 
xcodebuild -scheme Lni test -skipMacroValidation -destination 'platform=iOS Simulator,name=iPhone 15,OS=17.2' | xcbeautify