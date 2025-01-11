var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { LndNode } from "../../lni/pkg/bundler/lni.js";
function run() {
    return __awaiter(this, void 0, void 0, function* () {
        const node = new LndNode("test_macaroon", "https://127.0.0.1:8080");
        // Fetch wallet balance
        const res = node.pay_invoice("lno**");
        console.log("Wallet Balance:", res);
        const txn = node.get_wallet_transactions("wallet1");
        txn.forEach((t) => {
            console.log("Transaction:", t.amount, t.date, t.memo);
        });
        node.on_payment_received("lni1234", (result) => {
            console.log("Callback result:", result);
        });
    });
}
run();
