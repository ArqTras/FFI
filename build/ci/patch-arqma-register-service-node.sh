#!/usr/bin/env bash
# Expose service-node registration on wallet2_api (create_register_service_node_tx + commit_tx).
# Idempotent; applied after clone-arqma in FFI / desktop CI builds.
set -eu
UP="${1:-}"
if [[ -z "$UP" ]]; then
  echo "usage: $0 <arqma-upstream-root>" >&2
  exit 1
fi

API_H="$UP/src/wallet/api/wallet2_api.h"
WALLET_H="$UP/src/wallet/api/wallet.h"
WALLET_CPP="$UP/src/wallet/api/wallet.cpp"

for f in "$API_H" "$WALLET_H" "$WALLET_CPP"; do
  [[ -f "$f" ]] || exit 0
done

if grep -q "Arqma-GUI-MM: registerServiceNode" "$WALLET_CPP"; then
  exit 0
fi

perl -0777 -i -pe '
  if (!/registerServiceNode/) {
    s/(virtual PendingTransaction\* stakePending\(const std::string& service_node_key, const std::string& amount, std::string& error_msg\) = 0;\n)/$1\n    \/\/ Arqma-GUI-MM: registerServiceNode\n    virtual bool registerServiceNode(const std::string& register_service_node_str, std::string& error_msg) = 0;\n/s;
  }
' "$API_H"

perl -0777 -i -pe '
  if (!/registerServiceNode/) {
    s/(PendingTransaction\* stakePending\(const std::string& service_node_key, const std::string& amount, std::string& error_msg\) override;\n)/$1    bool registerServiceNode(const std::string& register_service_node_str, std::string& error_msg) override;\n/s;
  }
' "$WALLET_H"

perl -0777 -i -pe '
  if (!/Arqma-GUI-MM: registerServiceNode/) {
    s/(#include "cryptonote_core\/service_node_rules.h"\n)/$1#include <boost\/algorithm\/string.hpp>\n/s;
    s/(  return transaction;\n\}\n\n\} \/\/ namespace\n)/bool WalletImpl::registerServiceNode(const std::string& register_service_node_str, std::string& error_msg)\n{\n  std::vector<std::string> args;\n  boost::split(args, register_service_node_str, boost::is_any_of(" "));\n  if (!args.empty() && args[0] == "register_service_node") {\n    args.erase(args.begin());\n  }\n\n  tools::wallet2::register_service_node_result register_result =\n      m_wallet->create_register_service_node_tx(args, 0);\n  if (register_result.status != tools::wallet2::register_service_node_result_status::success) {\n    error_msg = register_result.msg;\n    LOG_ERROR("registerServiceNode failed: " << error_msg);\n    return false;\n  }\n\n  try {\n    std::vector<tools::wallet2::pending_tx> ptx_vector = {register_result.ptx};\n    m_wallet->commit_tx(ptx_vector);\n  } catch (const std::exception& e) {\n    error_msg = e.what();\n    LOG_ERROR("registerServiceNode commit failed: " << error_msg);\n    return false;\n  }\n\n  return true;\n}\n\n$1/s;
  }
' "$WALLET_CPP"

echo "[patch-arqma-register-service-node] patched wallet2_api (registerServiceNode)"
