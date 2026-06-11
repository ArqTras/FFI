#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#include "rust/cxx.h"

namespace Monero {
struct WalletManagerBase;
struct Wallet;
struct PendingTransaction;
}

struct Wallet2Bridge {
  Monero::WalletManagerBase* manager = nullptr;
  Monero::Wallet* wallet = nullptr;
  std::unordered_map<std::string, Monero::PendingTransaction*> pending_by_metadata;
  /// Last `exportPendingRelaySlices` / `do_not_relay` prepare — kept until `relay_tx` matches a slice
  /// hex, then submitted via `PendingTransaction::commit` (avoids fragile `relayTxFromMetadataHex`
  /// round-trip for multi-slice transfers).
  Monero::PendingTransaction* pending_relay_bundle = nullptr;
  std::vector<std::string> pending_relay_bundle_hexes;
  ~Wallet2Bridge();
};

std::unique_ptr<Wallet2Bridge> wallet2_open(
  const std::string& path,
  const std::string& password,
  const std::string& daemon,
  std::uint8_t network
);
// Initialize a bare bridge with only the wallet manager (no openWallet call).
// Used before `create_wallet` / `restore_deterministic_wallet` / `generate_from_keys`
// when the wallet does not yet exist on disk and we must NOT touch the filesystem.
std::unique_ptr<Wallet2Bridge> wallet2_init_bare();
void wallet2_store(Wallet2Bridge& bridge);
void wallet2_close(Wallet2Bridge& bridge);
rust::String wallet2_address(const Wallet2Bridge& bridge);
rust::String wallet2_seed(const Wallet2Bridge& bridge);
rust::String wallet2_secret_spend_key(const Wallet2Bridge& bridge);
rust::String wallet2_secret_view_key(const Wallet2Bridge& bridge);
bool wallet2_set_password(Wallet2Bridge& bridge, const std::string& new_password);
bool wallet2_set_tx_note(Wallet2Bridge& bridge, const std::string& txid, const std::string& note);
bool wallet2_export_key_images(const Wallet2Bridge& bridge, const std::string& filename);
bool wallet2_add_address_book(Wallet2Bridge& bridge, const std::string& address, const std::string& payment_id, const std::string& description);
bool wallet2_delete_address_book(Wallet2Bridge& bridge, std::uint64_t row_id);
rust::String wallet2_get_address_book_json(const Wallet2Bridge& bridge);
rust::String wallet2_get_transfer_by_txid_json(const Wallet2Bridge& bridge, const std::string& txid);
void wallet2_restore_deterministic_wallet(
  Wallet2Bridge& bridge,
  const std::string& path,
  const std::string& password,
  const std::string& seed,
  std::uint64_t restore_height,
  std::uint8_t network,
  const std::string& daemon
);
void wallet2_generate_from_keys(
  Wallet2Bridge& bridge,
  const std::string& path,
  const std::string& password,
  const std::string& language,
  std::uint64_t restore_height,
  const std::string& address,
  const std::string& view_key,
  const std::string& spend_key,
  std::uint8_t network,
  const std::string& daemon
);
void wallet2_create_wallet(
  Wallet2Bridge& bridge,
  const std::string& path,
  const std::string& password,
  const std::string& language,
  std::uint8_t network,
  const std::string& daemon
);
bool wallet2_rescan_blockchain(Wallet2Bridge& bridge);
void wallet2_rescan_blockchain_async(Wallet2Bridge& bridge);
bool wallet2_rescan_spent(Wallet2Bridge& bridge);
/// Synchronous refresh (matches `arqma-wallet-rpc` `refresh`): pull new blocks / txs from the daemon.
bool wallet2_refresh(Wallet2Bridge& bridge);
/// Sync refresh from [start_height] (subprocess wallet-rpc `refresh` parity when scan stalls).
bool wallet2_refresh_from_height(Wallet2Bridge& bridge, std::uint64_t start_height);
/// Non-blocking refresh: pause background refresh, optionally rewind scan height, `refreshAsync` + `startRefresh`.
void wallet2_refresh_async_start(
  Wallet2Bridge& bridge,
  std::uint64_t start_height,
  bool use_start_height
);
/// Wallet processed height and daemon chain tip (for UI progress while syncing).
void wallet2_read_scan_heights(
  const Wallet2Bridge& bridge,
  std::uint64_t& wallet_height,
  std::uint64_t& daemon_height
);
/// Stop wallet2 background refresh thread before other wallet2 calls (Flutter FFI scan completion).
void wallet2_pause_refresh(Wallet2Bridge& bridge);
bool wallet2_import_key_images(const Wallet2Bridge& bridge, const std::string& filename);
rust::String wallet2_stake_prepare_json(
  Wallet2Bridge& bridge,
  const std::string& service_node_key,
  const std::string& amount
);
rust::String wallet2_sweep_all_prepare_json(
  Wallet2Bridge& bridge,
  const std::string& address,
  bool do_not_relay
);
rust::String wallet2_relay_tx_json(Wallet2Bridge& bridge, const std::string& metadata_hex);
rust::String wallet2_get_accounts_json(const Wallet2Bridge& bridge, std::uint32_t account_tag);
rust::String wallet2_create_address_json(Wallet2Bridge& bridge, std::uint32_t account_index, const std::string& label);
rust::String wallet2_validate_address_json(const Wallet2Bridge& bridge, const std::string& address, bool any_net_type, bool allow_openalias);
rust::String wallet2_transfer_split_prepare_json(
  Wallet2Bridge& bridge,
  const std::string& address,
  const std::string& payment_id,
  std::uint64_t amount,
  std::uint32_t priority,
  bool do_not_relay
);
rust::String wallet2_get_transfers_json(
  const Wallet2Bridge& bridge,
  bool in_flag,
  bool out_flag,
  bool pending_flag,
  bool failed_flag,
  bool pool_flag,
  std::uint64_t min_height,
  std::uint64_t max_height
);
rust::String wallet2_register_service_node_json(Wallet2Bridge& bridge, const std::string& register_service_node_str);
rust::String wallet2_can_request_stake_unlock_json(Wallet2Bridge& bridge, const std::string& service_node_key);
rust::String wallet2_request_stake_unlock_json(Wallet2Bridge& bridge, const std::string& service_node_key);
std::uint64_t wallet2_height(const Wallet2Bridge& bridge);
std::uint64_t wallet2_balance(const Wallet2Bridge& bridge);
std::uint64_t wallet2_unlocked_balance(const Wallet2Bridge& bridge);
