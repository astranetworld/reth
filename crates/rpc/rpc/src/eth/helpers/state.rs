//! Contains RPC handler implementations specific to state.

use reth_provider::{ChainSpecProvider, StateProviderFactory};
use reth_transaction_pool::TransactionPool;

use reth_rpc_eth_api::helpers::{EthState, LoadState, SpawnBlocking};
use reth_rpc_eth_types::EthStateCache;

use crate::EthApi;

impl<Provider, Pool, Network, EvmConfig> EthState for EthApi<Provider, Pool, Network, EvmConfig>
where
    Self: LoadState + SpawnBlocking,
{
    fn max_proof_window(&self) -> u64 {
        self.inner.eth_proof_window()
    }
}

impl<Provider, Pool, Network, EvmConfig> LoadState for EthApi<Provider, Pool, Network, EvmConfig>
where
    Self: Send + Sync,
    Provider: StateProviderFactory + ChainSpecProvider,
    Pool: TransactionPool,
{
    #[inline]
    fn provider(&self) -> impl StateProviderFactory + ChainSpecProvider {
        self.inner.provider()
    }

    #[inline]
    fn cache(&self) -> &EthStateCache {
        self.inner.cache()
    }

    #[inline]
    fn pool(&self) -> impl TransactionPool {
        self.inner.pool()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_evm_ethereum::EthEvmConfig;
    use reth_primitives::{
        constants::ETHEREUM_BLOCK_GAS_LIMIT, Address, StorageKey, StorageValue, KECCAK_EMPTY, U256,
    };
    use reth_provider::test_utils::{ExtendedAccount, MockEthProvider, NoopProvider};
    use reth_rpc_eth_api::helpers::EthState;
    use reth_rpc_eth_types::{
        EthStateCache, FeeHistoryCache, FeeHistoryCacheConfig, GasPriceOracle,
    };
    use reth_rpc_server_types::constants::{DEFAULT_ETH_PROOF_WINDOW, DEFAULT_PROOF_PERMITS};
    use reth_tasks::pool::BlockingTaskPool;
    use reth_transaction_pool::test_utils::{testing_pool, TestPool};
    use std::collections::HashMap;

    fn noop_eth_api() -> EthApi<NoopProvider, TestPool, (), EthEvmConfig> {
        let pool = testing_pool();
        let evm_config = EthEvmConfig::default();

        let cache = EthStateCache::spawn(NoopProvider::default(), Default::default(), evm_config);
        EthApi::new(
            NoopProvider::default(),
            pool,
            (),
            cache.clone(),
            GasPriceOracle::new(NoopProvider::default(), Default::default(), cache.clone()),
            ETHEREUM_BLOCK_GAS_LIMIT,
            DEFAULT_ETH_PROOF_WINDOW,
            BlockingTaskPool::build().expect("failed to build tracing pool"),
            FeeHistoryCache::new(cache, FeeHistoryCacheConfig::default()),
            evm_config,
            None,
            DEFAULT_PROOF_PERMITS,
        )
    }

    fn mock_eth_api(
        accounts: HashMap<Address, ExtendedAccount>,
    ) -> EthApi<MockEthProvider, TestPool, (), EthEvmConfig> {
        let pool = testing_pool();
        let evm_config = EthEvmConfig::default();

        let mock_provider = MockEthProvider::default();
        mock_provider.extend_accounts(accounts);

        let cache = EthStateCache::spawn(mock_provider.clone(), Default::default(), evm_config);
        EthApi::new(
            mock_provider.clone(),
            pool,
            (),
            cache.clone(),
            GasPriceOracle::new(mock_provider, Default::default(), cache.clone()),
            ETHEREUM_BLOCK_GAS_LIMIT,
            DEFAULT_ETH_PROOF_WINDOW,
            BlockingTaskPool::build().expect("failed to build tracing pool"),
            FeeHistoryCache::new(cache, FeeHistoryCacheConfig::default()),
            evm_config,
            None,
            DEFAULT_PROOF_PERMITS,
        )
    }

    #[tokio::test]
    async fn test_storage() {
        // === Noop ===
        let eth_api = noop_eth_api();
        let address = Address::random();
        let storage = eth_api.storage_at(address, U256::ZERO.into(), None).await.unwrap();
        assert_eq!(storage, U256::ZERO.to_be_bytes());

        // === Mock ===
        let storage_value = StorageValue::from(1337);
        let storage_key = StorageKey::random();
        let storage = HashMap::from([(storage_key, storage_value)]);

        let accounts =
            HashMap::from([(address, ExtendedAccount::new(0, U256::ZERO).extend_storage(storage))]);
        let eth_api = mock_eth_api(accounts);

        let storage_key: U256 = storage_key.into();
        let storage = eth_api.storage_at(address, storage_key.into(), None).await.unwrap();
        assert_eq!(storage, storage_value.to_be_bytes());
    }

    #[tokio::test]
    async fn test_get_account_missing() {
        let eth_api = noop_eth_api();
        let address = Address::random();
        let account = eth_api.get_account(address, Default::default()).await.unwrap();
        assert!(account.is_none());
    }

    #[tokio::test]
    async fn test_get_account_empty() {
        let address = Address::random();
        let accounts = HashMap::from([(address, ExtendedAccount::new(0, U256::ZERO))]);
        let eth_api = mock_eth_api(accounts);

        let account = eth_api.get_account(address, Default::default()).await.unwrap();
        let expected_account =
            reth_rpc_types::Account { code_hash: KECCAK_EMPTY, ..Default::default() };
        assert_eq!(Some(expected_account), account);
    }
}
