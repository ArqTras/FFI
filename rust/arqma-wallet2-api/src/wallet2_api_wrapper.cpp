#include "wallet2_api_wrapper.hpp"

#include <cctype>
#include <cinttypes>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <memory>
#include <random>
#include <stdexcept>
#include <string>
#include <sstream>
#include <unordered_map>
#include <vector>

#include "wallet2_api.h"

namespace {
Monero::NetworkType to_network(const std::uint8_t network) {
  switch (network) {
    case 1:
      return Monero::TESTNET;
    case 2:
      return Monero::STAGENET;
    case 0:
    default:
      return Monero::MAINNET;
  }
}

static std::string json_escape(const std::string& in) {
  std::string out;
  out.reserve(in.size() + 8);
  for (char c : in) {
    switch (c) {
      case '\\': out += "\\\\"; break;
      case '"': out += "\\\""; break;
      case '\n': out += "\\n"; break;
      case '\r': out += "\\r"; break;
      case '\t': out += "\\t"; break;
      default: out.push_back(c); break;
    }
  }
  return out;
}

/// Trim ASCII whitespace at both ends (multisig / relay keys must match UI round-trips).
static std::string trim_copy(const std::string& in) {
  size_t start = 0;
  while (start < in.size() &&
         std::isspace(static_cast<unsigned char>(in[start]))) {
    ++start;
  }
  size_t end = in.size();
  while (end > start &&
         std::isspace(static_cast<unsigned char>(in[end - 1]))) {
    --end;
  }
  return in.substr(start, end - start);
}

/// Strip `0x`, trim, remove whitespace — `relayTxFromMetadataHex` / epee hex parsers expect raw nibbles.
static std::string hex_compact_relay_metadata(const std::string& in) {
  std::string s = trim_copy(in);
  if (s.size() >= 2 && s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
    s.erase(0, 2);
  }
  std::string out;
  out.reserve(s.size());
  for (unsigned char c : s) {
    if (!std::isspace(c)) {
      out.push_back(static_cast<char>(c));
    }
  }
  return out;
}

static bool hex_metadata_looks_valid(const std::string& hex) {
  if (hex.empty() || (hex.size() % 2) != 0) {
    return false;
  }
  for (unsigned char c : hex) {
    if (!std::isxdigit(c)) {
      return false;
    }
  }
  return true;
}

/// Normalize hex for obsolete file-relay path (no `relayTxFromMetadataHex` in headers).
#if !defined(ARQMA_WALLET2_HAS_RELAY_FROM_HEX)
static std::string hex_compact_for_unsigned_relay(const std::string& in) {
  return hex_compact_relay_metadata(in);
}
#endif

#if !defined(ARQMA_WALLET2_HAS_SLICE_RELAY) && !defined(ARQMA_WALLET2_HAS_EXPORT_PENDING_RELAY)
static std::filesystem::path make_temp_unsigned_tx_path() {
  std::random_device rd;
  std::mt19937 gen(rd());
  std::uniform_int_distribution<unsigned> d(0, 15);
  static constexpr char kHex[] = "0123456789abcdef";
  std::string suffix(16, '0');
  for (char& c : suffix) {
    c = kHex[d(gen)];
  }
  return std::filesystem::temp_directory_path() /
         ("arqma_wallet2_unsigned_" + suffix + ".tx");
}

static std::string legacy_hex_encode(const std::string& raw) {
  static constexpr char kHex[] = "0123456789abcdef";
  std::string out;
  out.reserve(raw.size() * 2);
  for (unsigned char c : raw) {
    out.push_back(kHex[c >> 4]);
    out.push_back(kHex[c & 0xf]);
  }
  return out;
}

#if !defined(ARQMA_WALLET2_HAS_RELAY_FROM_HEX)
static bool legacy_hex_decode(const std::string& hex, std::string& out) {
  out.clear();
  if (hex.size() % 2 != 0) {
    return false;
  }
  auto xd = [](char x) -> int {
    if (x >= '0' && x <= '9') {
      return x - '0';
    }
    if (x >= 'a' && x <= 'f') {
      return x - 'a' + 10;
    }
    if (x >= 'A' && x <= 'F') {
      return x - 'A' + 10;
    }
    return -1;
  };
  for (size_t i = 0; i + 1 < hex.size(); i += 2) {
    const int hi = xd(hex[i]);
    const int lo = xd(hex[i + 1]);
    if (hi < 0 || lo < 0) {
      return false;
    }
    out.push_back(static_cast<char>((hi << 4) | lo));
  }
  return true;
}
#endif

static bool legacy_read_file_binary(const std::filesystem::path& path, std::string& out) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    return false;
  }
  f.seekg(0, std::ios::end);
  const auto sz = f.tellg();
  if (sz < 0) {
    return false;
  }
  f.seekg(0, std::ios::beg);
  out.resize(static_cast<size_t>(sz));
  if (sz > 0) {
    f.read(out.data(), sz);
  }
  return static_cast<bool>(f);
}

#if !defined(ARQMA_WALLET2_HAS_RELAY_FROM_HEX)
static bool legacy_write_file_binary(const std::filesystem::path& path, const std::string& data) {
  std::ofstream f(path, std::ios::binary | std::ios::trunc);
  if (!f) {
    return false;
  }
  if (!data.empty()) {
    f.write(data.data(), static_cast<std::streamsize>(data.size()));
  }
  return static_cast<bool>(f);
}
#endif

static std::string legacy_export_unsigned_pending_hex(Monero::PendingTransaction* ptx) {
  const auto path = make_temp_unsigned_tx_path();
  const std::string path_str = path.string();
  if (!ptx->commit(path_str, true)) {
    std::error_code ec;
    std::filesystem::remove(path, ec);
    throw std::runtime_error("prepare: failed to export unsigned transaction");
  }
  std::string raw;
  if (!legacy_read_file_binary(path, raw) || raw.empty()) {
    std::error_code ec;
    std::filesystem::remove(path, ec);
    throw std::runtime_error("prepare: empty unsigned transaction blob");
  }
  std::error_code ec;
  std::filesystem::remove(path, ec);
  return legacy_hex_encode(raw);
}

#endif  // legacy unsigned file export (no portable_binary export path)

}  // namespace

static void wallet2_clear_pending_relay_bundle(Wallet2Bridge& bridge) {
  if (bridge.wallet != nullptr && bridge.pending_relay_bundle != nullptr) {
    bridge.wallet->disposeTransaction(bridge.pending_relay_bundle);
  }
  bridge.pending_relay_bundle = nullptr;
  bridge.pending_relay_bundle_hexes.clear();
}

static void wallet2_assign_pending_relay_bundle(
    Wallet2Bridge& bridge,
    Monero::PendingTransaction* ptx,
    std::vector<std::string> hexes) {
  wallet2_clear_pending_relay_bundle(bridge);
  bridge.pending_relay_bundle = ptx;
  bridge.pending_relay_bundle_hexes = std::move(hexes);
}

Wallet2Bridge::~Wallet2Bridge() {
  if (wallet != nullptr) {
    wallet2_clear_pending_relay_bundle(*this);
    for (auto& kv : pending_by_metadata) {
      if (kv.second != nullptr) {
        wallet->disposeTransaction(kv.second);
      }
    }
    pending_by_metadata.clear();
  }
  if (manager != nullptr && wallet != nullptr) {
    manager->closeWallet(wallet, true);
    wallet = nullptr;
  }
}

void clear_pending(Wallet2Bridge& bridge) {
  wallet2_clear_pending_relay_bundle(bridge);
  if (bridge.wallet == nullptr) {
    return;
  }
  for (auto& kv : bridge.pending_by_metadata) {
    if (kv.second != nullptr) {
      bridge.wallet->disposeTransaction(kv.second);
    }
  }
  bridge.pending_by_metadata.clear();
}

std::unique_ptr<Wallet2Bridge> wallet2_open(
  const std::string& path,
  const std::string& password,
  const std::string& daemon,
  std::uint8_t network
) {
  auto bridge = std::make_unique<Wallet2Bridge>();
  bridge->manager = Monero::WalletManagerFactory::getWalletManager();
  if (bridge->manager == nullptr) {
    throw std::runtime_error("WalletManagerFactory::getWalletManager returned null");
  }

  std::string path_s(path);
  std::string pass_s(password);
  std::string daemon_s(daemon);
  bridge->wallet = bridge->manager->openWallet(path_s, pass_s, to_network(network));
  if (bridge->wallet == nullptr) {
    throw std::runtime_error("openWallet returned null");
  }
  // `WalletManagerImpl::openWallet` ALWAYS returns a non-null wallet, even after a
  // failed load (e.g. wallet does not exist on disk yet). The wallet ends up in
  // `Status_Critical` and a subsequent `init(daemon)` would CLEAR that status, leaving
  // a broken session that would later create empty wallet cache files on `closeWallet`.
  // Detect the failure here and roll back without touching the filesystem.
  if (bridge->wallet->status() != Monero::Wallet::Status_Ok) {
    std::string err = bridge->wallet->errorString();
    bridge->manager->closeWallet(bridge->wallet, false);
    bridge->wallet = nullptr;
    throw std::runtime_error(err.empty() ? "openWallet failed" : err);
  }

  if (!daemon_s.empty()) {
    bridge->wallet->init(daemon_s);
    bridge->wallet->startRefresh();
  }
  return bridge;
}

std::unique_ptr<Wallet2Bridge> wallet2_init_bare() {
  auto bridge = std::make_unique<Wallet2Bridge>();
  bridge->manager = Monero::WalletManagerFactory::getWalletManager();
  if (bridge->manager == nullptr) {
    throw std::runtime_error("WalletManagerFactory::getWalletManager returned null");
  }
  return bridge;
}

void wallet2_store(Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  if (!bridge.wallet->store("")) {
    throw std::runtime_error(bridge.wallet->errorString());
  }
}

void wallet2_close(Wallet2Bridge& bridge) {
  if (bridge.manager == nullptr || bridge.wallet == nullptr) {
    return;
  }
  clear_pending(bridge);
  if (!bridge.manager->closeWallet(bridge.wallet, true)) {
    throw std::runtime_error(bridge.manager->errorString());
  }
  bridge.wallet = nullptr;
}

rust::String wallet2_address(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return rust::String(bridge.wallet->address());
}

rust::String wallet2_seed(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return rust::String(bridge.wallet->seed());
}

rust::String wallet2_secret_spend_key(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return rust::String(bridge.wallet->secretSpendKey());
}

rust::String wallet2_secret_view_key(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return rust::String(bridge.wallet->secretViewKey());
}

bool wallet2_set_password(Wallet2Bridge& bridge, const std::string& new_password) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->setPassword(new_password);
}

bool wallet2_set_tx_note(Wallet2Bridge& bridge, const std::string& txid, const std::string& note) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->setUserNote(txid, note);
}

bool wallet2_export_key_images(const Wallet2Bridge& bridge, const std::string& filename) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->exportKeyImages(filename);
}

bool wallet2_add_address_book(Wallet2Bridge& bridge, const std::string& address, const std::string& payment_id, const std::string& description) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  auto* ab = bridge.wallet->addressBook();
  if (ab == nullptr) {
    throw std::runtime_error("address book is null");
  }
  return ab->addRow(address, payment_id, description);
}

bool wallet2_delete_address_book(Wallet2Bridge& bridge, std::uint64_t row_id) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  auto* ab = bridge.wallet->addressBook();
  if (ab == nullptr) {
    throw std::runtime_error("address book is null");
  }
  return ab->deleteRow(static_cast<std::size_t>(row_id));
}

rust::String wallet2_get_address_book_json(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  auto* ab = bridge.wallet->addressBook();
  if (ab == nullptr) {
    return rust::String("{\"entries\":[]}");
  }
  ab->refresh();
  const auto rows = ab->getAll();
  std::ostringstream oss;
  oss << "{\"entries\":[";
  bool first = true;
  for (const auto* row : rows) {
    if (!row) continue;
    if (!first) oss << ",";
    first = false;
    oss << "{"
        << "\"index\":" << row->getRowId() << ","
        << "\"address\":\"" << json_escape(row->getAddress()) << "\","
        << "\"payment_id\":\"" << json_escape(row->getPaymentId()) << "\","
        << "\"description\":\"" << json_escape(row->getDescription()) << "\""
        << "}";
  }
  oss << "]}";
  return rust::String(oss.str());
}

rust::String wallet2_get_transfer_by_txid_json(const Wallet2Bridge& bridge, const std::string& txid) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  auto* hist = bridge.wallet->history();
  if (hist == nullptr) {
    return rust::String("{}");
  }
  auto* tr = hist->transaction(txid);
  if (tr == nullptr) {
    return rust::String("{}");
  }
  std::ostringstream oss;
  oss << "{"
      << "\"txid\":\"" << json_escape(tr->hash()) << "\","
      << "\"amount\":" << tr->amount() << ","
      << "\"fee\":" << tr->fee() << ","
      << "\"height\":" << tr->blockHeight() << ","
      << "\"timestamp\":" << static_cast<std::uint64_t>(tr->timestamp())
      << "}";
  return rust::String(oss.str());
}

void wallet2_restore_deterministic_wallet(
  Wallet2Bridge& bridge,
  const std::string& path,
  const std::string& password,
  const std::string& seed,
  std::uint64_t restore_height,
  std::uint8_t network,
  const std::string& daemon
) {
  if (bridge.manager == nullptr) {
    throw std::runtime_error("wallet manager is null");
  }
  if (bridge.wallet != nullptr) {
    clear_pending(bridge);
    bridge.manager->closeWallet(bridge.wallet, true);
    bridge.wallet = nullptr;
  }
  std::string path_s(path);
  std::string pass_s(password);
  std::string seed_s(seed);
  std::string daemon_s(daemon);
  bridge.wallet = bridge.manager->recoveryWallet(
    path_s,
    pass_s,
    seed_s,
    to_network(network),
    restore_height
  );
  if (bridge.wallet == nullptr) {
    throw std::runtime_error(bridge.manager->errorString());
  }
  if (bridge.wallet->status() != Monero::Wallet::Status_Ok) {
    throw std::runtime_error(bridge.wallet->errorString());
  }
  if (!daemon_s.empty()) {
    bridge.wallet->init(daemon_s);
    bridge.wallet->startRefresh();
  }
}

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
) {
  if (bridge.manager == nullptr) {
    throw std::runtime_error("wallet manager is null");
  }
  if (bridge.wallet != nullptr) {
    clear_pending(bridge);
    bridge.manager->closeWallet(bridge.wallet, true);
    bridge.wallet = nullptr;
  }
  std::string path_s(path);
  std::string pass_s(password);
  std::string lang_s(language);
  std::string addr_s(address);
  std::string view_s(view_key);
  std::string spend_s(spend_key);
  std::string daemon_s(daemon);
  bridge.wallet = bridge.manager->createWalletFromKeys(
    path_s,
    pass_s,
    lang_s,
    to_network(network),
    restore_height,
    addr_s,
    view_s,
    spend_s
  );
  if (bridge.wallet == nullptr) {
    throw std::runtime_error(bridge.manager->errorString());
  }
  if (bridge.wallet->status() != Monero::Wallet::Status_Ok) {
    throw std::runtime_error(bridge.wallet->errorString());
  }
  if (!daemon_s.empty()) {
    bridge.wallet->init(daemon_s);
    bridge.wallet->startRefresh();
  }
}

void wallet2_create_wallet(
  Wallet2Bridge& bridge,
  const std::string& path,
  const std::string& password,
  const std::string& language,
  std::uint8_t network,
  const std::string& daemon
) {
  if (bridge.manager == nullptr) {
    throw std::runtime_error("wallet manager is null");
  }
  if (bridge.wallet != nullptr) {
    clear_pending(bridge);
    bridge.manager->closeWallet(bridge.wallet, true);
    bridge.wallet = nullptr;
  }
  std::string path_s(path);
  std::string pass_s(password);
  std::string lang_s(language);
  std::string daemon_s(daemon);
  bridge.wallet = bridge.manager->createWallet(path_s, pass_s, lang_s, to_network(network));
  if (bridge.wallet == nullptr) {
    throw std::runtime_error(bridge.manager->errorString());
  }
  if (bridge.wallet->status() != Monero::Wallet::Status_Ok) {
    throw std::runtime_error(bridge.wallet->errorString());
  }
  if (!daemon_s.empty()) {
    bridge.wallet->init(daemon_s);
    bridge.wallet->startRefresh();
  }
}

bool wallet2_rescan_blockchain(Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->rescanBlockchain();
}

bool wallet2_rescan_spent(Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->rescanSpent();
}

bool wallet2_refresh(Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->refresh();
}

bool wallet2_refresh_from_height(Wallet2Bridge& bridge, std::uint64_t start_height) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  // Stall recovery: background `startRefresh` can hold the refresh mutex while height stops
  // advancing — `refresh()` then returns false. Pause the thread, sync from height, resume.
  bridge.wallet->pauseRefresh();
  bridge.wallet->setRefreshFromBlockHeight(start_height);
  bool ok = bridge.wallet->refresh();
  if (!ok) {
    const int conn = static_cast<int>(bridge.wallet->connected());
    const std::uint64_t dh = bridge.wallet->daemonBlockChainHeight();
    const std::uint64_t wh = bridge.wallet->blockChainHeight();
    std::fprintf(
        stderr,
        "[wallet2] refresh_from_height(%" PRIu64 "): sync refresh false connected=%d "
        "daemon_h=%" PRIu64 " wallet_h=%" PRIu64 "\n",
        start_height,
        conn,
        dh,
        wh);
    bridge.wallet->refreshAsync();
    ok = true;
  }
  bridge.wallet->startRefresh();
  return ok;
}

bool wallet2_import_key_images(const Wallet2Bridge& bridge, const std::string& filename) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->importKeyImages(filename);
}

rust::String wallet2_stake_prepare_json(
    Wallet2Bridge& bridge,
    const std::string& service_node_key,
    const std::string& amount
) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  wallet2_clear_pending_relay_bundle(bridge);
  std::string snk(service_node_key);
  std::string amt(amount);
  std::string err;
  Monero::PendingTransaction* ptx = bridge.wallet->stakePending(snk, amt, err);
  if (ptx == nullptr) {
    throw std::runtime_error(err.empty() ? "stakePending failed" : err);
  }
  if (ptx->status() != Monero::PendingTransaction::Status_Ok) {
    std::string e = ptx->errorString();
    bridge.wallet->disposeTransaction(ptx);
    throw std::runtime_error(e.empty() ? "stake pending status error" : e);
  }
  const uint64_t fee = ptx->fee();
  std::string metadata;
  if (bridge.wallet->multisig().isMultisig) {
    metadata = ptx->multisigSignData();
    if (metadata.empty()) {
      bridge.wallet->disposeTransaction(ptx);
      throw std::runtime_error("stake: empty tx metadata");
    }
    metadata = trim_copy(metadata);
    bridge.pending_by_metadata[metadata] = ptx;
  } else {
#if defined(ARQMA_WALLET2_HAS_EXPORT_PENDING_RELAY)
    std::vector<std::string> hexes;
    std::vector<uint64_t> fees;
    if (!bridge.wallet->exportPendingRelaySlices(ptx, hexes, fees) ||
        hexes.size() != 1u ||
        hexes[0].empty()) {
      bridge.wallet->disposeTransaction(ptx);
      throw std::runtime_error("stake: empty tx metadata");
    }
    metadata = hexes[0];
    wallet2_assign_pending_relay_bundle(bridge, ptx, std::move(hexes));
#else
    try {
      metadata = legacy_export_unsigned_pending_hex(ptx);
    } catch (const std::runtime_error&) {
      bridge.wallet->disposeTransaction(ptx);
      throw;
    }
    bridge.wallet->disposeTransaction(ptx);
#endif
  }
  std::ostringstream oss;
  oss << "{"
      << "\"tx_metadata\":\"" << json_escape(metadata) << "\","
      << "\"fee\":" << fee
      << "}";
  return rust::String(oss.str());
}

rust::String wallet2_sweep_all_prepare_json(
    Wallet2Bridge& bridge,
    const std::string& address,
    bool do_not_relay
) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  wallet2_clear_pending_relay_bundle(bridge);
  std::string dst(address);
  Monero::optional<uint64_t> amount;
  std::set<uint32_t> subaddr_indices;
  Monero::PendingTransaction* ptx = bridge.wallet->createTransaction(
    dst,
    "",
    amount,
    0,
    Monero::PendingTransaction::Priority_Low,
    0,
    subaddr_indices
  );
  if (ptx == nullptr) {
    throw std::runtime_error("sweep_all: createTransaction returned null");
  }
  if (ptx->status() != Monero::PendingTransaction::Status_Ok) {
    std::string e = ptx->errorString();
    bridge.wallet->disposeTransaction(ptx);
    throw std::runtime_error(e.empty() ? "sweep_all pending status error" : e);
  }
  std::vector<std::string> txids = ptx->txid();
  const std::string txh = txids.empty() ? "" : txids.front();
  const uint64_t fee = ptx->fee();
  if (do_not_relay) {
    if (bridge.wallet->multisig().isMultisig) {
      const std::string metadata = trim_copy(ptx->multisigSignData());
      if (metadata.empty()) {
        bridge.wallet->disposeTransaction(ptx);
        throw std::runtime_error("sweep_all: empty tx metadata");
      }
      bridge.pending_by_metadata[metadata] = ptx;
      std::ostringstream oss;
      oss << "{"
          << "\"tx_metadata_list\":[\"" << json_escape(metadata) << "\"],"
          << "\"tx_hash_list\":[\"" << json_escape(txh) << "\"],"
          << "\"fee_list\":[" << fee << "]"
          << "}";
      return rust::String(oss.str());
    }
#if defined(ARQMA_WALLET2_HAS_EXPORT_PENDING_RELAY)
    std::vector<std::string> hexes;
    std::vector<uint64_t> fees;
    if (!bridge.wallet->exportPendingRelaySlices(ptx, hexes, fees)) {
      bridge.wallet->disposeTransaction(ptx);
      throw std::runtime_error("sweep_all: empty tx metadata");
    }
    txids = ptx->txid();
    if (hexes.empty() || hexes.size() != fees.size() || hexes.size() != txids.size()) {
      bridge.wallet->disposeTransaction(ptx);
      throw std::runtime_error("sweep_all: empty tx metadata");
    }
    for (const std::string& h : hexes) {
      if (h.empty()) {
        bridge.wallet->disposeTransaction(ptx);
        throw std::runtime_error("sweep_all: empty tx metadata");
      }
    }
    std::ostringstream oss;
    oss << "{\"tx_metadata_list\":[";
    for (size_t i = 0; i < hexes.size(); ++i) {
      if (i > 0) oss << ',';
      oss << '"' << json_escape(hexes[i]) << '"';
    }
    oss << "],\"tx_hash_list\":[";
    for (size_t i = 0; i < txids.size(); ++i) {
      if (i > 0) oss << ',';
      oss << '"' << json_escape(txids[i]) << '"';
    }
    oss << "],\"fee_list\":[";
    for (size_t i = 0; i < fees.size(); ++i) {
      if (i > 0) oss << ',';
      oss << fees[i];
    }
    oss << "]}";
    wallet2_assign_pending_relay_bundle(bridge, ptx, std::move(hexes));
    return rust::String(oss.str());
#else
    std::string combined_hex;
    try {
      combined_hex = legacy_export_unsigned_pending_hex(ptx);
    } catch (const std::runtime_error&) {
      bridge.wallet->disposeTransaction(ptx);
      throw;
    }
    txids = ptx->txid();
    const size_t n = txids.empty() ? 1 : txids.size();
    const uint64_t base_fee = n ? fee / static_cast<uint64_t>(n) : 0;
    const uint64_t fee_rem = n ? fee % static_cast<uint64_t>(n) : 0;
    std::ostringstream oss;
    oss << "{\"tx_metadata_list\":[\"" << json_escape(combined_hex) << "\"],\"tx_hash_list\":[";
    for (size_t i = 0; i < txids.size(); ++i) {
      if (i > 0) oss << ',';
      oss << '"' << json_escape(txids[i]) << '"';
    }
    oss << "],\"fee_list\":[";
    for (size_t i = 0; i < n; ++i) {
      if (i > 0) oss << ',';
      const uint64_t fi = base_fee + (i == 0 ? fee_rem : 0);
      oss << fi;
    }
    oss << "]}";
    bridge.wallet->disposeTransaction(ptx);
    return rust::String(oss.str());
#endif
  }
  if (!ptx->commit()) {
    std::string e = ptx->errorString();
    bridge.wallet->disposeTransaction(ptx);
    throw std::runtime_error(e.empty() ? "sweep_all commit failed" : e);
  }
  bridge.wallet->disposeTransaction(ptx);
  std::ostringstream oss;
  oss << "{"
      << "\"tx_hash_list\":[\"" << json_escape(txh) << "\"],"
      << "\"fee_list\":[" << fee << "]"
      << "}";
  return rust::String(oss.str());
}

rust::String wallet2_relay_tx_json(Wallet2Bridge& bridge, const std::string& metadata_hex) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  const std::string relay_trimmed = trim_copy(metadata_hex);
  auto it = bridge.pending_by_metadata.find(metadata_hex);
  if (it == bridge.pending_by_metadata.end() && relay_trimmed != metadata_hex) {
    it = bridge.pending_by_metadata.find(relay_trimmed);
  }
  if (it != bridge.pending_by_metadata.end() && it->second != nullptr) {
    Monero::PendingTransaction* ptx = it->second;
    std::vector<std::string> txids = ptx->txid();
    const std::string txh = txids.empty() ? "" : txids.front();
    if (!ptx->commit()) {
      throw std::runtime_error(ptx->errorString());
    }
    bridge.wallet->disposeTransaction(ptx);
    bridge.pending_by_metadata.erase(it);
    std::ostringstream oss;
    oss << "{"
        << "\"tx_hash\":\"" << json_escape(txh) << "\""
        << "}";
    return rust::String(oss.str());
  }
  {
    const std::string relay_try = hex_compact_relay_metadata(metadata_hex);
    if (bridge.pending_relay_bundle != nullptr) {
      bool matched = false;
      for (const auto& h : bridge.pending_relay_bundle_hexes) {
        if (hex_compact_relay_metadata(h) == relay_try) {
          matched = true;
          break;
        }
      }
      if (matched) {
        Monero::PendingTransaction* bundle = bridge.pending_relay_bundle;
        const std::vector<std::string> txids_before = bundle->txid();
        bridge.pending_relay_bundle = nullptr;
        bridge.pending_relay_bundle_hexes.clear();
        const bool ok = bundle->commit("");
        const std::string cerr = bundle->errorString();
        bridge.wallet->disposeTransaction(bundle);
        if (!ok) {
          throw std::runtime_error(cerr.empty() ? "relay_tx: pending bundle commit failed" : cerr);
        }
        const std::string txh = txids_before.empty() ? "" : txids_before.front();
        std::ostringstream oss;
        oss << "{"
            << "\"tx_hash\":\"" << json_escape(txh) << "\""
            << ",\"relay_committed_all_pending_slices\":true"
            << "}";
        return rust::String(oss.str());
      }
    }
  }
#if defined(ARQMA_WALLET2_HAS_RELAY_FROM_HEX)
  {
    const std::string relay_hex = hex_compact_relay_metadata(metadata_hex);
    if (!hex_metadata_looks_valid(relay_hex)) {
      throw std::runtime_error(
          "relay_tx: invalid transaction metadata hex (expect even-length [0-9A-Fa-f], optional 0x "
          "prefix, no non-hex characters)");
    }
    std::string txh;
    if (!bridge.wallet->relayTxFromMetadataHex(relay_hex, txh)) {
      const std::string werr = bridge.wallet->errorString();
      int st = 0;
      std::string st_msg;
      bridge.wallet->statusWithErrorString(st, st_msg);
      const int conn = static_cast<int>(bridge.wallet->connected());
      const uint64_t dh = bridge.wallet->daemonBlockChainHeight();
      const uint64_t wh = bridge.wallet->blockChainHeight();
      std::ostringstream detail;
      detail << "relay_tx: relayTxFromMetadataHex failed (metadata_hex_chars=" << relay_hex.size() << ")";
      if (!werr.empty()) {
        detail << " [" << werr << "]";
      } else if (!st_msg.empty()) {
        detail << " [wallet_status=" << st << " " << st_msg << "]";
      }
      detail << " [daemon_connect_status=" << conn << " (1=Connected) daemon_blockchain_height=" << dh
             << " wallet_blockchain_height=" << wh << "]";
      if (werr.empty() && st_msg.empty()) {
        detail << " Upstream `WalletImpl::relayTxFromMetadataHex` returns false without setting "
                  "errorString when hex parse fails, boost::archive deserialize fails, or commit_tx throws. "
                  "Check: arqmad reachable (height>0), same Arqma `pospow` build for libwallet + headers as "
                  "when the unsigned blob was created, daemon logs for sendrawtransaction, and whether this "
                  "slice was already relayed (duplicate / stale pending blob).";
      }
      throw std::runtime_error(detail.str());
    }
    std::ostringstream oss;
    oss << "{"
        << "\"tx_hash\":\"" << json_escape(txh) << "\""
        << "}";
    return rust::String(oss.str());
  }
#endif

#if !defined(ARQMA_WALLET2_HAS_RELAY_FROM_HEX)
  std::string raw;
  const std::string hex_for_decode = hex_compact_for_unsigned_relay(metadata_hex);
  if (!legacy_hex_decode(hex_for_decode, raw) || raw.empty()) {
    throw std::runtime_error("relay_tx: invalid hex metadata");
  }
  const auto path = std::filesystem::absolute(make_temp_unsigned_tx_path());
  if (!legacy_write_file_binary(path, raw)) {
    throw std::runtime_error("relay_tx: failed to write temp transaction file");
  }
  const bool ok = bridge.wallet->submitTransaction(path.string());
  std::error_code ec;
  std::filesystem::remove(path, ec);
  if (!ok) {
    const std::string werr = bridge.wallet->errorString();
    const std::string msg =
        werr.empty() ? "relay_tx: submitTransaction failed" : ("relay_tx: " + werr);
    throw std::runtime_error(msg);
  }
  return rust::String("{\"tx_hash\":\"\"}");
#endif  // !ARQMA_WALLET2_HAS_RELAY_FROM_HEX
}

rust::String wallet2_get_accounts_json(const Wallet2Bridge& bridge, std::uint32_t) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  auto* sa = bridge.wallet->subaddressAccount();
  if (sa == nullptr) {
    return rust::String("{\"subaddress_accounts\":[]}");
  }
  sa->refresh();
  auto rows = sa->getAll();
  std::ostringstream oss;
  oss << "{\"subaddress_accounts\":[";
  bool first = true;
  for (const auto* row : rows) {
    if (!row) continue;
    if (!first) oss << ",";
    first = false;
    oss << "{"
        << "\"account_index\":" << row->getRowId() << ","
        << "\"base_address\":\"" << json_escape(row->getAddress()) << "\","
        << "\"label\":\"" << json_escape(row->getLabel()) << "\","
        << "\"balance\":" << row->getBalance() << ","
        << "\"unlocked_balance\":" << row->getUnlockedBalance()
        << "}";
  }
  oss << "]}";
  return rust::String(oss.str());
}

rust::String wallet2_create_address_json(Wallet2Bridge& bridge, std::uint32_t account_index, const std::string& label) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  bridge.wallet->addSubaddress(account_index, label);
  const auto count = bridge.wallet->numSubaddresses(account_index);
  if (count == 0) {
    throw std::runtime_error("create_address: no subaddresses");
  }
  const auto new_index = static_cast<uint32_t>(count - 1);
  auto* s = bridge.wallet->subaddress();
  if (s != nullptr) {
    s->refresh(account_index);
    const auto all = s->getAll();
    for (const auto* row : all) {
      if (!row) continue;
      if (row->getRowId() == new_index) {
        std::ostringstream oss;
        oss << "{"
            << "\"address\":\"" << json_escape(row->getAddress()) << "\","
            << "\"address_index\":" << new_index
            << "}";
        return rust::String(oss.str());
      }
    }
  }
  std::ostringstream oss;
  oss << "{"
      << "\"address\":\"\","
      << "\"address_index\":" << new_index
      << "}";
  return rust::String(oss.str());
}

rust::String wallet2_validate_address_json(const Wallet2Bridge& bridge, const std::string& address, bool any_net_type, bool) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  const std::string addr(address);
  const auto net = bridge.wallet->nettype();
  bool valid = Monero::Wallet::addressValid(addr, net);
  if (!valid && any_net_type) {
    valid = Monero::Wallet::addressValid(addr, Monero::MAINNET)
      || Monero::Wallet::addressValid(addr, Monero::TESTNET)
      || Monero::Wallet::addressValid(addr, Monero::STAGENET);
  }
  std::ostringstream oss;
  oss << "{"
      << "\"valid\":" << (valid ? "true" : "false") << ","
      << "\"integrated\":" << "false" << ","
      << "\"subaddress\":" << "false" << ","
      << "\"nettype\":" << static_cast<int>(net)
      << "}";
  return rust::String(oss.str());
}

rust::String wallet2_transfer_split_prepare_json(
    Wallet2Bridge& bridge,
    const std::string& address,
    const std::string& payment_id,
    std::uint64_t amount,
    std::uint32_t priority,
    bool do_not_relay
) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  wallet2_clear_pending_relay_bundle(bridge);
  std::string dst(address);
  const std::string pid = trim_copy(payment_id);
  Monero::optional<uint64_t> amt(amount);
  std::set<uint32_t> subaddr_indices;
  auto pri = Monero::PendingTransaction::Priority_Low;
  switch (priority) {
    case 2: pri = Monero::PendingTransaction::Priority_Medium; break;
    case 3: pri = Monero::PendingTransaction::Priority_High; break;
    case 0:
    case 1:
    default: pri = Monero::PendingTransaction::Priority_Low; break;
  }
  Monero::PendingTransaction* ptx = bridge.wallet->createTransaction(
    dst,
    pid,
    amt,
    0,
    pri,
    0,
    subaddr_indices
  );
  if (ptx == nullptr) {
    throw std::runtime_error("transfer_split: createTransaction returned null");
  }
  if (ptx->status() != Monero::PendingTransaction::Status_Ok) {
    std::string e = ptx->errorString();
    bridge.wallet->disposeTransaction(ptx);
    throw std::runtime_error(e.empty() ? "transfer_split pending status error" : e);
  }
  std::vector<std::string> txids = ptx->txid();
  const std::string txh = txids.empty() ? "" : txids.front();
  const uint64_t fee = ptx->fee();
  if (do_not_relay) {
    if (bridge.wallet->multisig().isMultisig) {
      const std::string metadata = trim_copy(ptx->multisigSignData());
      if (metadata.empty()) {
        bridge.wallet->disposeTransaction(ptx);
        throw std::runtime_error("transfer_split: empty tx metadata");
      }
      bridge.pending_by_metadata[metadata] = ptx;
#if defined(ARQMA_WALLET2_HAS_SLICE_RELAY)
      const std::vector<uint64_t> slice_amts = ptx->destinationAmountsPerSlice();
      const uint64_t a0 = slice_amts.empty() ? 0 : slice_amts[0];
#else
      const uint64_t a0 = ptx->amount();
#endif
      std::ostringstream oss;
      oss << "{"
          << "\"tx_metadata_list\":[\"" << json_escape(metadata) << "\"],"
          << "\"tx_hash_list\":[\"" << json_escape(txh) << "\"],"
          << "\"fee_list\":[" << fee << "],"
          << "\"amount_list\":[" << a0 << "]"
          << "}";
      return rust::String(oss.str());
    }
#if defined(ARQMA_WALLET2_HAS_EXPORT_PENDING_RELAY)
    std::vector<std::string> hexes;
    std::vector<uint64_t> fees;
    if (!bridge.wallet->exportPendingRelaySlices(ptx, hexes, fees)) {
      bridge.wallet->disposeTransaction(ptx);
      throw std::runtime_error("transfer_split: empty tx metadata");
    }
    txids = ptx->txid();
    if (hexes.empty() || hexes.size() != fees.size() || hexes.size() != txids.size()) {
      bridge.wallet->disposeTransaction(ptx);
      throw std::runtime_error("transfer_split: empty tx metadata");
    }
    for (const std::string& h : hexes) {
      if (h.empty()) {
        bridge.wallet->disposeTransaction(ptx);
        throw std::runtime_error("transfer_split: empty tx metadata");
      }
    }
    std::ostringstream oss;
    oss << "{\"tx_metadata_list\":[";
    for (size_t i = 0; i < hexes.size(); ++i) {
      if (i > 0) oss << ',';
      oss << '"' << json_escape(hexes[i]) << '"';
    }
    oss << "],\"tx_hash_list\":[";
    for (size_t i = 0; i < txids.size(); ++i) {
      if (i > 0) oss << ',';
      oss << '"' << json_escape(txids[i]) << '"';
    }
#if defined(ARQMA_WALLET2_HAS_SLICE_RELAY)
    const std::vector<uint64_t> slice_amts = ptx->destinationAmountsPerSlice();
#endif
    oss << "],\"fee_list\":[";
    for (size_t i = 0; i < fees.size(); ++i) {
      if (i > 0) oss << ',';
      oss << fees[i];
    }
    oss << "],\"amount_list\":[";
    for (size_t i = 0; i < hexes.size(); ++i) {
      if (i > 0) oss << ',';
#if defined(ARQMA_WALLET2_HAS_SLICE_RELAY)
      const uint64_t a = (i < slice_amts.size()) ? slice_amts[i] : 0;
#else
      const uint64_t total_amt = ptx->amount();
      const size_t ns = hexes.size();
      const uint64_t base_amt = ns ? total_amt / static_cast<uint64_t>(ns) : 0;
      const uint64_t rem = ns ? total_amt % static_cast<uint64_t>(ns) : 0;
      const uint64_t a = base_amt + (i == 0 ? rem : 0);
#endif
      oss << a;
    }
    oss << "]}";
    wallet2_assign_pending_relay_bundle(bridge, ptx, std::move(hexes));
    return rust::String(oss.str());
#else
    std::string combined_hex;
    try {
      combined_hex = legacy_export_unsigned_pending_hex(ptx);
    } catch (const std::runtime_error&) {
      bridge.wallet->disposeTransaction(ptx);
      throw;
    }
    txids = ptx->txid();
    const size_t n = txids.empty() ? 1 : txids.size();
    const uint64_t total_amt = ptx->amount();
    const uint64_t base_amt = n ? total_amt / static_cast<uint64_t>(n) : 0;
    const uint64_t amt_rem = n ? total_amt % static_cast<uint64_t>(n) : 0;
    const uint64_t base_fee = n ? fee / static_cast<uint64_t>(n) : 0;
    const uint64_t fee_rem = n ? fee % static_cast<uint64_t>(n) : 0;
    std::ostringstream oss;
    oss << "{\"tx_metadata_list\":[\"" << json_escape(combined_hex) << "\"],\"tx_hash_list\":[";
    for (size_t i = 0; i < txids.size(); ++i) {
      if (i > 0) oss << ',';
      oss << '"' << json_escape(txids[i]) << '"';
    }
    oss << "],\"fee_list\":[";
    for (size_t i = 0; i < n; ++i) {
      if (i > 0) oss << ',';
      oss << (base_fee + (i == 0 ? fee_rem : 0));
    }
    oss << "],\"amount_list\":[";
    for (size_t i = 0; i < n; ++i) {
      if (i > 0) oss << ',';
      oss << (base_amt + (i == 0 ? amt_rem : 0));
    }
    oss << "]}";
    bridge.wallet->disposeTransaction(ptx);
    return rust::String(oss.str());
#endif
  }
  if (!ptx->commit()) {
    std::string e = ptx->errorString();
    bridge.wallet->disposeTransaction(ptx);
    throw std::runtime_error(e.empty() ? "transfer_split commit failed" : e);
  }
  bridge.wallet->disposeTransaction(ptx);
  std::ostringstream oss;
  oss << "{"
      << "\"tx_hash_list\":[\"" << json_escape(txh) << "\"],"
      << "\"fee_list\":[" << fee << "]"
      << "}";
  return rust::String(oss.str());
}

rust::String wallet2_get_transfers_json(
  const Wallet2Bridge& bridge,
  bool in_flag,
  bool out_flag,
  bool pending_flag,
  bool failed_flag,
  bool pool_flag,
  std::uint64_t min_height,
  std::uint64_t max_height
) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  auto* hist = bridge.wallet->history();
  if (hist == nullptr) {
    return rust::String("{\"in\":[],\"out\":[],\"pending\":[],\"failed\":[],\"pool\":[]}");
  }
  hist->refresh();
  const auto all = hist->getAll();
  std::ostringstream in_s, out_s, pending_s, failed_s, pool_s;
  in_s << "[";
  out_s << "[";
  pending_s << "[";
  failed_s << "[";
  pool_s << "[";
  bool in_first = true, out_first = true, pending_first = true, failed_first = true, pool_first = true;
  for (const auto* tx : all) {
    if (!tx) continue;
    const uint64_t h = tx->blockHeight();
    if (h < min_height || h > max_height) continue;
    // `type` mirrors `arqma-rpc-upstream::wallet2.h::pay_type_string` so the
    // Flutter/Tauri/Electron filters (Service Node / Miner / Stake / …) match.
    // `TransactionInfo` only exposes service-node and miner reward flags here,
    // so outgoing stakes remain `out` — same as upstream `make_transfer_view`
    // for `confirmed_transfer_details`.
    const bool is_in =
        tx->direction() == Monero::TransactionInfo::Direction_In;
    const char* pay_type_str = is_in ? "in" : "out";
    if (is_in) {
      if (tx->isServiceNodeReward()) {
        pay_type_str = "snode";
      } else if (tx->isMinerReward()) {
        pay_type_str = "miner";
      }
    }
    std::ostringstream row;
    row << "{"
        << "\"amount\":" << tx->amount() << ","
        << "\"fee\":" << tx->fee() << ","
        << "\"height\":" << h << ","
        << "\"timestamp\":" << static_cast<std::uint64_t>(tx->timestamp()) << ","
        << "\"txid\":\"" << json_escape(tx->hash()) << "\","
        << "\"payment_id\":\"" << json_escape(tx->paymentId()) << "\","
        << "\"type\":\"" << pay_type_str << "\""
        << "}";
    const std::string row_s = row.str();
    if (tx->isFailed()) {
      if (failed_flag) {
        if (!failed_first) failed_s << ",";
        failed_first = false;
        failed_s << row_s;
      }
      continue;
    }
    if (tx->isPending()) {
      if (pending_flag) {
        if (!pending_first) pending_s << ",";
        pending_first = false;
        pending_s << row_s;
      }
      if (pool_flag) {
        if (!pool_first) pool_s << ",";
        pool_first = false;
        pool_s << row_s;
      }
      continue;
    }
    if (tx->direction() == Monero::TransactionInfo::Direction_In) {
      if (in_flag) {
        if (!in_first) in_s << ",";
        in_first = false;
        in_s << row_s;
      }
    } else {
      if (out_flag) {
        if (!out_first) out_s << ",";
        out_first = false;
        out_s << row_s;
      }
    }
  }
  in_s << "]";
  out_s << "]";
  pending_s << "]";
  failed_s << "]";
  pool_s << "]";
  std::ostringstream oss;
  oss << "{"
      << "\"in\":" << in_s.str() << ","
      << "\"out\":" << out_s.str() << ","
      << "\"pending\":" << pending_s.str() << ","
      << "\"failed\":" << failed_s.str() << ","
      << "\"pool\":" << pool_s.str()
      << "}";
  return rust::String(oss.str());
}

rust::String wallet2_register_service_node_json(Wallet2Bridge& bridge, const std::string& register_service_node_str) {
  if (bridge.wallet == nullptr) {
    return rust::String("{\"error\":{\"code\":-32603,\"message\":\"wallet is null\"}}");
  }
#if !defined(ARQMA_WALLET2_HAS_REGISTER_SERVICE_NODE)
  (void)register_service_node_str;
  return rust::String(
      "{\"error\":{\"code\":-32603,\"message\":\"register_service_node not available in this native build\"}}");
#else
  std::string err;
  if (!bridge.wallet->registerServiceNode(register_service_node_str, err)) {
    std::ostringstream oss;
    oss << "{\"error\":{\"code\":-32603,\"message\":\"" << json_escape(err) << "\"}}";
    return rust::String(oss.str());
  }
  return rust::String("{}");
#endif
}

rust::String wallet2_can_request_stake_unlock_json(Wallet2Bridge& bridge, const std::string& service_node_key) {
  (void) bridge;
  (void) service_node_key;
  return rust::String("{\"can_unlock\":false,\"msg\":\"can_request_stake_unlock unavailable in current native build\"}");
}

rust::String wallet2_request_stake_unlock_json(Wallet2Bridge& bridge, const std::string& service_node_key) {
  (void) bridge;
  (void) service_node_key;
  return rust::String("{\"unlocked\":false,\"msg\":\"request_stake_unlock unavailable in current native build\"}");
}

std::uint64_t wallet2_height(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->blockChainHeight();
}

std::uint64_t wallet2_balance(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->balanceAll();
}

std::uint64_t wallet2_unlocked_balance(const Wallet2Bridge& bridge) {
  if (bridge.wallet == nullptr) {
    throw std::runtime_error("wallet is null");
  }
  return bridge.wallet->unlockedBalanceAll();
}

// `cryptonote_core` references `windows::check_admin` from daemonizer. MinGW CI adds daemonizer to
// `wallet_merged` (patch-arqma-mingw-gui); only provide a stub when that TU is not in the merged lib.
#if defined(WIN32) && !defined(ARQMA_WALLET2_DAEMONIZER_IN_MERGED)
#undef UNICODE
#undef _UNICODE
#include <windows.h>
namespace windows {
bool check_admin(bool& result) {
  BOOL is_member = FALSE;
  PSID admin_group = nullptr;
  SID_IDENTIFIER_AUTHORITY nt_auth = SECURITY_NT_AUTHORITY;
  if (AllocateAndInitializeSid(&nt_auth, 2, SECURITY_BUILTIN_DOMAIN_RID,
          DOMAIN_ALIAS_RID_ADMINS, 0, 0, 0, 0, 0, 0, &admin_group)) {
    CheckTokenMembership(nullptr, admin_group, &is_member);
    FreeSid(admin_group);
  }
  result = is_member != FALSE;
  return true;
}
}
#endif
