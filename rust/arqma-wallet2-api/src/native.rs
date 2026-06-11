use cxx::{let_cxx_string, UniquePtr};

use crate::{NetworkKind, Wallet2Balance, Wallet2Error, Wallet2OpenConfig, Wallet2Result};

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("wallet2_api_wrapper.hpp");

        type Wallet2Bridge;

        fn wallet2_open(
            path: &CxxString,
            password: &CxxString,
            daemon: &CxxString,
            network: u8,
        ) -> Result<UniquePtr<Wallet2Bridge>>;
        fn wallet2_init_bare() -> Result<UniquePtr<Wallet2Bridge>>;
        fn wallet2_store(bridge: Pin<&mut Wallet2Bridge>) -> Result<()>;
        fn wallet2_close(bridge: Pin<&mut Wallet2Bridge>) -> Result<()>;
        fn wallet2_address(bridge: &Wallet2Bridge) -> Result<String>;
        fn wallet2_seed(bridge: &Wallet2Bridge) -> Result<String>;
        fn wallet2_secret_spend_key(bridge: &Wallet2Bridge) -> Result<String>;
        fn wallet2_secret_view_key(bridge: &Wallet2Bridge) -> Result<String>;
        fn wallet2_set_password(
            bridge: Pin<&mut Wallet2Bridge>,
            new_password: &CxxString,
        ) -> Result<bool>;
        fn wallet2_set_tx_note(
            bridge: Pin<&mut Wallet2Bridge>,
            txid: &CxxString,
            note: &CxxString,
        ) -> Result<bool>;
        fn wallet2_export_key_images(bridge: &Wallet2Bridge, filename: &CxxString) -> Result<bool>;
        fn wallet2_add_address_book(
            bridge: Pin<&mut Wallet2Bridge>,
            address: &CxxString,
            payment_id: &CxxString,
            description: &CxxString,
        ) -> Result<bool>;
        fn wallet2_delete_address_book(
            bridge: Pin<&mut Wallet2Bridge>,
            row_id: u64,
        ) -> Result<bool>;
        fn wallet2_get_address_book_json(bridge: &Wallet2Bridge) -> Result<String>;
        fn wallet2_get_transfer_by_txid_json(
            bridge: &Wallet2Bridge,
            txid: &CxxString,
        ) -> Result<String>;
        fn wallet2_restore_deterministic_wallet(
            bridge: Pin<&mut Wallet2Bridge>,
            path: &CxxString,
            password: &CxxString,
            seed: &CxxString,
            restore_height: u64,
            network: u8,
            daemon: &CxxString,
        ) -> Result<()>;
        fn wallet2_generate_from_keys(
            bridge: Pin<&mut Wallet2Bridge>,
            path: &CxxString,
            password: &CxxString,
            language: &CxxString,
            restore_height: u64,
            address: &CxxString,
            view_key: &CxxString,
            spend_key: &CxxString,
            network: u8,
            daemon: &CxxString,
        ) -> Result<()>;
        fn wallet2_create_wallet(
            bridge: Pin<&mut Wallet2Bridge>,
            path: &CxxString,
            password: &CxxString,
            language: &CxxString,
            network: u8,
            daemon: &CxxString,
        ) -> Result<()>;
        fn wallet2_rescan_blockchain(bridge: Pin<&mut Wallet2Bridge>) -> Result<bool>;
        fn wallet2_rescan_blockchain_async(bridge: Pin<&mut Wallet2Bridge>) -> Result<()>;
        fn wallet2_rescan_spent(bridge: Pin<&mut Wallet2Bridge>) -> Result<bool>;
        fn wallet2_refresh(bridge: Pin<&mut Wallet2Bridge>) -> Result<bool>;
        fn wallet2_refresh_from_height(
            bridge: Pin<&mut Wallet2Bridge>,
            start_height: u64,
        ) -> Result<bool>;
        fn wallet2_refresh_async_start(
            bridge: Pin<&mut Wallet2Bridge>,
            start_height: u64,
            use_start_height: bool,
        ) -> Result<()>;
        fn wallet2_read_scan_heights(
            bridge: &Wallet2Bridge,
            wallet_height: &mut u64,
            daemon_height: &mut u64,
        ) -> Result<()>;
        fn wallet2_pause_refresh(bridge: Pin<&mut Wallet2Bridge>) -> Result<()>;
        fn wallet2_import_key_images(bridge: &Wallet2Bridge, filename: &CxxString) -> Result<bool>;
        fn wallet2_stake_prepare_json(
            bridge: Pin<&mut Wallet2Bridge>,
            service_node_key: &CxxString,
            amount: &CxxString,
        ) -> Result<String>;
        fn wallet2_sweep_all_prepare_json(
            bridge: Pin<&mut Wallet2Bridge>,
            address: &CxxString,
            do_not_relay: bool,
        ) -> Result<String>;
        fn wallet2_relay_tx_json(
            bridge: Pin<&mut Wallet2Bridge>,
            metadata_hex: &CxxString,
        ) -> Result<String>;
        fn wallet2_get_accounts_json(bridge: &Wallet2Bridge, account_tag: u32) -> Result<String>;
        fn wallet2_create_address_json(
            bridge: Pin<&mut Wallet2Bridge>,
            account_index: u32,
            label: &CxxString,
        ) -> Result<String>;
        fn wallet2_validate_address_json(
            bridge: &Wallet2Bridge,
            address: &CxxString,
            any_net_type: bool,
            allow_openalias: bool,
        ) -> Result<String>;
        fn wallet2_transfer_split_prepare_json(
            bridge: Pin<&mut Wallet2Bridge>,
            address: &CxxString,
            payment_id: &CxxString,
            amount: u64,
            priority: u32,
            do_not_relay: bool,
        ) -> Result<String>;
        fn wallet2_get_transfers_json(
            bridge: &Wallet2Bridge,
            in_flag: bool,
            out_flag: bool,
            pending_flag: bool,
            failed_flag: bool,
            pool_flag: bool,
            min_height: u64,
            max_height: u64,
        ) -> Result<String>;
        fn wallet2_register_service_node_json(
            bridge: Pin<&mut Wallet2Bridge>,
            register_service_node_str: &CxxString,
        ) -> Result<String>;
        fn wallet2_can_request_stake_unlock_json(
            bridge: Pin<&mut Wallet2Bridge>,
            service_node_key: &CxxString,
        ) -> Result<String>;
        fn wallet2_request_stake_unlock_json(
            bridge: Pin<&mut Wallet2Bridge>,
            service_node_key: &CxxString,
        ) -> Result<String>;
        fn wallet2_height(bridge: &Wallet2Bridge) -> Result<u64>;
        fn wallet2_balance(bridge: &Wallet2Bridge) -> Result<u64>;
        fn wallet2_unlocked_balance(bridge: &Wallet2Bridge) -> Result<u64>;
    }
}

fn net_to_u8(n: &NetworkKind) -> u8 {
    match n {
        NetworkKind::Mainnet => 0,
        NetworkKind::Testnet => 1,
        NetworkKind::Stagenet => 2,
    }
}

pub struct NativeWallet2Session {
    bridge: UniquePtr<ffi::Wallet2Bridge>,
}

unsafe impl Send for NativeWallet2Session {}
unsafe impl Sync for NativeWallet2Session {}

impl NativeWallet2Session {
    pub fn open(cfg: &Wallet2OpenConfig) -> Wallet2Result<Self> {
        let_cxx_string!(path = cfg.wallet_path.as_str());
        let_cxx_string!(password = cfg.password.as_str());
        let_cxx_string!(daemon = cfg.daemon_address.as_str());
        let bridge = ffi::wallet2_open(&path, &password, &daemon, net_to_u8(&cfg.network))
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))?;
        Ok(Self { bridge })
    }

    pub fn bare() -> Wallet2Result<Self> {
        let bridge =
            ffi::wallet2_init_bare().map_err(|e| Wallet2Error::OperationFailed(e.to_string()))?;
        Ok(Self { bridge })
    }

    pub fn store(&mut self) -> Wallet2Result<()> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_store(b.as_mut()).map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn close(&mut self) -> Wallet2Result<()> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_close(b.as_mut()).map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn height(&self) -> Wallet2Result<u64> {
        ffi::wallet2_height(&self.bridge).map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn address(&self) -> Wallet2Result<String> {
        ffi::wallet2_address(&self.bridge).map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn seed(&self) -> Wallet2Result<String> {
        ffi::wallet2_seed(&self.bridge).map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn secret_spend_key(&self) -> Wallet2Result<String> {
        ffi::wallet2_secret_spend_key(&self.bridge)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn secret_view_key(&self) -> Wallet2Result<String> {
        ffi::wallet2_secret_view_key(&self.bridge)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn set_password(&mut self, new_password: &str) -> Wallet2Result<bool> {
        let_cxx_string!(pass = new_password);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_set_password(b.as_mut(), &pass)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn set_tx_note(&mut self, txid: &str, note: &str) -> Wallet2Result<bool> {
        let_cxx_string!(tx = txid);
        let_cxx_string!(nt = note);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_set_tx_note(b.as_mut(), &tx, &nt)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn export_key_images(&self, filename: &str) -> Wallet2Result<bool> {
        let_cxx_string!(f = filename);
        ffi::wallet2_export_key_images(&self.bridge, &f)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn add_address_book(
        &mut self,
        address: &str,
        payment_id: &str,
        description: &str,
    ) -> Wallet2Result<bool> {
        let_cxx_string!(a = address);
        let_cxx_string!(p = payment_id);
        let_cxx_string!(d = description);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_add_address_book(b.as_mut(), &a, &p, &d)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn delete_address_book(&mut self, row_id: u64) -> Wallet2Result<bool> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_delete_address_book(b.as_mut(), row_id)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn get_address_book_json(&self) -> Wallet2Result<String> {
        ffi::wallet2_get_address_book_json(&self.bridge)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn get_transfer_by_txid_json(&self, txid: &str) -> Wallet2Result<String> {
        let_cxx_string!(t = txid);
        ffi::wallet2_get_transfer_by_txid_json(&self.bridge, &t)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn restore_deterministic_wallet(
        &mut self,
        path: &str,
        password: &str,
        seed: &str,
        restore_height: u64,
        network: &NetworkKind,
        daemon: &str,
    ) -> Wallet2Result<()> {
        let_cxx_string!(p = path);
        let_cxx_string!(pw = password);
        let_cxx_string!(s = seed);
        let_cxx_string!(d = daemon);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_restore_deterministic_wallet(
            b.as_mut(),
            &p,
            &pw,
            &s,
            restore_height,
            net_to_u8(network),
            &d,
        )
        .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn generate_from_keys(
        &mut self,
        path: &str,
        password: &str,
        language: &str,
        restore_height: u64,
        address: &str,
        view_key: &str,
        spend_key: &str,
        network: &NetworkKind,
        daemon: &str,
    ) -> Wallet2Result<()> {
        let_cxx_string!(p = path);
        let_cxx_string!(pw = password);
        let_cxx_string!(lang = language);
        let_cxx_string!(addr = address);
        let_cxx_string!(view = view_key);
        let_cxx_string!(spend = spend_key);
        let_cxx_string!(d = daemon);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_generate_from_keys(
            b.as_mut(),
            &p,
            &pw,
            &lang,
            restore_height,
            &addr,
            &view,
            &spend,
            net_to_u8(network),
            &d,
        )
        .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn create_wallet(
        &mut self,
        path: &str,
        password: &str,
        language: &str,
        network: &NetworkKind,
        daemon: &str,
    ) -> Wallet2Result<()> {
        let_cxx_string!(p = path);
        let_cxx_string!(pw = password);
        let_cxx_string!(lang = language);
        let_cxx_string!(d = daemon);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_create_wallet(b.as_mut(), &p, &pw, &lang, net_to_u8(network), &d)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn rescan_blockchain(&mut self) -> Wallet2Result<bool> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_rescan_blockchain(b.as_mut())
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn rescan_blockchain_async(&mut self) -> Wallet2Result<()> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_rescan_blockchain_async(b.as_mut())
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn rescan_spent(&mut self) -> Wallet2Result<bool> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_rescan_spent(b.as_mut())
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn refresh(&mut self) -> Wallet2Result<bool> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_refresh(b.as_mut())
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn refresh_from_height(&mut self, start_height: u64) -> Wallet2Result<bool> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_refresh_from_height(b.as_mut(), start_height)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn refresh_async_start(&mut self, start_height: Option<u64>) -> Wallet2Result<()> {
        let mut b = self.bridge.pin_mut();
        let (height, use_start) = match start_height {
            Some(h) => (h, true),
            None => (0, false),
        };
        ffi::wallet2_refresh_async_start(b.as_mut(), height, use_start)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn scan_heights(&self) -> Wallet2Result<(u64, u64)> {
        let mut wallet_height: u64 = 0;
        let mut daemon_height: u64 = 0;
        ffi::wallet2_read_scan_heights(
            &self.bridge,
            &mut wallet_height,
            &mut daemon_height,
        )
        .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))?;
        Ok((wallet_height, daemon_height))
    }

    pub fn pause_refresh(&mut self) -> Wallet2Result<()> {
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_pause_refresh(b.as_mut())
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn import_key_images(&self, filename: &str) -> Wallet2Result<bool> {
        let_cxx_string!(f = filename);
        ffi::wallet2_import_key_images(&self.bridge, &f)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn stake_prepare_json(
        &mut self,
        service_node_key: &str,
        amount: &str,
    ) -> Wallet2Result<String> {
        let_cxx_string!(k = service_node_key);
        let_cxx_string!(a = amount);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_stake_prepare_json(b.as_mut(), &k, &a)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn sweep_all_prepare_json(
        &mut self,
        address: &str,
        do_not_relay: bool,
    ) -> Wallet2Result<String> {
        let_cxx_string!(addr = address);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_sweep_all_prepare_json(b.as_mut(), &addr, do_not_relay)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn relay_tx_json(&mut self, metadata_hex: &str) -> Wallet2Result<String> {
        let_cxx_string!(m = metadata_hex);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_relay_tx_json(b.as_mut(), &m)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn get_accounts_json(&self, account_tag: u32) -> Wallet2Result<String> {
        ffi::wallet2_get_accounts_json(&self.bridge, account_tag)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn create_address_json(
        &mut self,
        account_index: u32,
        label: &str,
    ) -> Wallet2Result<String> {
        let_cxx_string!(l = label);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_create_address_json(b.as_mut(), account_index, &l)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn validate_address_json(
        &self,
        address: &str,
        any_net_type: bool,
        allow_openalias: bool,
    ) -> Wallet2Result<String> {
        let_cxx_string!(a = address);
        ffi::wallet2_validate_address_json(&self.bridge, &a, any_net_type, allow_openalias)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn transfer_split_prepare_json(
        &mut self,
        address: &str,
        payment_id: &str,
        amount: u64,
        priority: u32,
        do_not_relay: bool,
    ) -> Wallet2Result<String> {
        let_cxx_string!(a = address);
        let_cxx_string!(pid = payment_id);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_transfer_split_prepare_json(
            b.as_mut(),
            &a,
            &pid,
            amount,
            priority,
            do_not_relay,
        )
        .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn get_transfers_json(
        &self,
        in_flag: bool,
        out_flag: bool,
        pending_flag: bool,
        failed_flag: bool,
        pool_flag: bool,
        min_height: u64,
        max_height: u64,
    ) -> Wallet2Result<String> {
        ffi::wallet2_get_transfers_json(
            &self.bridge,
            in_flag,
            out_flag,
            pending_flag,
            failed_flag,
            pool_flag,
            min_height,
            max_height,
        )
        .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn register_service_node_json(
        &mut self,
        register_service_node_str: &str,
    ) -> Wallet2Result<String> {
        let_cxx_string!(s = register_service_node_str);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_register_service_node_json(b.as_mut(), &s)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn can_request_stake_unlock_json(
        &mut self,
        service_node_key: &str,
    ) -> Wallet2Result<String> {
        let_cxx_string!(k = service_node_key);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_can_request_stake_unlock_json(b.as_mut(), &k)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn request_stake_unlock_json(&mut self, service_node_key: &str) -> Wallet2Result<String> {
        let_cxx_string!(k = service_node_key);
        let mut b = self.bridge.pin_mut();
        ffi::wallet2_request_stake_unlock_json(b.as_mut(), &k)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))
    }

    pub fn balance(&self) -> Wallet2Result<Wallet2Balance> {
        let balance = ffi::wallet2_balance(&self.bridge)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))?;
        let unlocked_balance = ffi::wallet2_unlocked_balance(&self.bridge)
            .map_err(|e| Wallet2Error::OperationFailed(e.to_string()))?;
        Ok(Wallet2Balance {
            balance,
            unlocked_balance,
        })
    }
}
