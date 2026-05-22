//! Canonical JSON-RPC `method` names for the native `Wallet2ApiClient`.
//!
//! Upstream `arqma-wallet-rpc` mixes legacy names (`getbalance`) with Monero-style underscores
//! (`get_balance`). External tools may send either; the GUI uses legacy spellings.

/// Map alternate spellings to the method string handled in
/// [`crate::wallet2_client::Wallet2ApiClient::call_json`].
///
/// Names mirror **`ArqTras/arqma` `pospow`** `MAP_JON_RPC_WE` in
/// `src/wallet/wallet_rpc_server.h` (legacy spellings + Monero-style underscores).
pub(crate) fn canonical_wallet_rpc_method(method: &str) -> &str {
    match method {
        "get_balance" => "getbalance",
        "getaddress" => "get_address",
        "get_height" => "getheight",
        "save" => "store",
        "close" => "close_wallet",
        "stop" => "stop_wallet",
        "rescan_bc" | "rescanblockchain" => "rescan_blockchain",
        "relay" => "relay_tx",
        "submit_transfer" => "transfer",
        _ => method,
    }
}
