import Foundation

// Exercise the real UniFFI boundary without credentials or network requests.
@main
struct OnchainRecordSmoke {
    static func main() async throws {
        let node = StrikeNode(config: StrikeConfig(
            baseUrl: "http://127.0.0.1:1", apiKey: "unused-test-key"
        ))
        for supported: Bool? in [nil, false, true] {
            let transaction = OnchainTransaction(
                id: nil,
                feeLimitSupported: supported,
                address: "test-address",
                amountSats: 10_000,
                feePayer: .sender,
                fee: OnchainFeePreference(preferenceType: .default)
            )
            let roundTrip = try FfiConverterTypeOnchainTransaction_lift(
                FfiConverterTypeOnchainTransaction_lower(transaction)
            )
            precondition(roundTrip == transaction, "Swift record round-trip failed")
            do {
                // Rust rejects the missing quote ID before constructing an HTTP client.
                _ = try await node.payOnchain(transaction: transaction)
                preconditionFailure("Expected missing quote ID to be rejected")
            } catch ApiError.InvalidInput(let reason) {
                precondition(reason == "pay_onchain requires an on-chain transaction id")
            }
        }
        print("Swift on-chain records decode correctly in Rust for nil/false/true fee limits.")
    }
}
