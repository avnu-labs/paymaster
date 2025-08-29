use std::ops::Deref;
use std::time::Duration;

use async_trait::async_trait;
use paymaster_common::service::fallback::{Error, FailurePredicate, WithFallback};
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    ConfirmedBlockId, ContractClass, ContractStorageKeys, DeclareTransactionResult, DeployAccountTransactionResult, EventFilter, EventsPage, FeeEstimate, Felt,
    FunctionCall, Hash256, InvokeTransactionResult, MaybePreConfirmedBlockWithReceipts, MaybePreConfirmedBlockWithTxHashes, MaybePreConfirmedBlockWithTxs,
    MaybePreConfirmedStateUpdate, MessageFeeEstimate, MessageStatus, MsgFromL1, SimulatedTransaction, SimulationFlag, SimulationFlagForEstimateFee, StorageProof,
    SyncStatusType, Transaction, TransactionReceiptWithBlockInfo, TransactionStatus, TransactionTrace, TransactionTraceWithHash,
};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClientError};
use starknet::providers::{JsonRpcClient, Provider, ProviderError, ProviderRequestData, ProviderResponseData, Url};
use tracing::instrument;

macro_rules! call_with_fallback {
    ($self: ident . $method: ident ( $($arg: expr),* )) => {
        $self
            .0
            .call(|x| async move { x.$method( $($arg),* ).await })
            .await
            .map_err(|e| match e {
                Error::Inner(e) => {
                    tracing::warn!("{}", e);
                    e
                },
                Error::Rejected => {
                    tracing::warn!("{}", e);
                    ProviderError::Other(Box::new(JsonRpcClientError::TransportError(e)))
                }
            })
    };
}

#[derive(Clone)]
struct StarknetRPCClient(JsonRpcClient<HttpTransport>);

impl StarknetRPCClient {
    fn new(endpoint: &str, timeout: u64) -> Self {
        Url::parse(endpoint)
            .map(|endpoint| {
                HttpTransport::new_with_client(
                    endpoint,
                    reqwest::Client::builder()
                        .timeout(Duration::from_secs(timeout))
                        .connect_timeout(Duration::from_secs(5))
                        .tcp_keepalive(Some(Duration::from_secs(30)))
                        .build()
                        .expect("failed to build Starknet HTTP client"),
                )
            })
            .map(JsonRpcClient::new)
            .map(Self)
            .expect("invalid client")
    }
}

impl Deref for StarknetRPCClient {
    type Target = JsonRpcClient<HttpTransport>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FailurePredicate<ProviderError> for StarknetRPCClient {
    fn is_err(&self, err: &ProviderError) -> bool {
        match err {
            ProviderError::RateLimited => true,
            ProviderError::Other(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct StarknetClient(WithFallback<StarknetRPCClient>);

impl StarknetClient {
    pub fn new(endpoint: &str, timeout: u64) -> Self {
        Self(WithFallback::new().with(StarknetRPCClient::new(endpoint, timeout)))
    }

    pub fn with_fallback(mut self, endpoint: &str, timeout: u64) -> Self {
        self.0 = self.0.with(StarknetRPCClient::new(endpoint, timeout));
        self
    }
}

#[async_trait]
impl Provider for StarknetClient {
    #[instrument(name = "spec_version", skip(self))]
    async fn spec_version(&self) -> Result<String, ProviderError> {
        call_with_fallback!(self.spec_version())
    }

    /// Gets block information with transaction hashes given the block id.
    #[instrument(name = "get_block_with_tx_hashes", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn get_block_with_tx_hashes<B>(&self, block_id: B) -> Result<MaybePreConfirmedBlockWithTxHashes, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_block_with_tx_hashes(block_id))
    }

    /// Gets block information with full transactions given the block id.
    #[instrument(name = "get_block_with_txs", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn get_block_with_txs<B>(&self, block_id: B) -> Result<MaybePreConfirmedBlockWithTxs, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_block_with_txs(block_id))
    }

    /// Gets block information with full transactions and receipts given the block id.
    #[instrument(name = "get_block_with_receipts", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn get_block_with_receipts<B>(&self, block_id: B) -> Result<MaybePreConfirmedBlockWithReceipts, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_block_with_receipts(block_id))
    }

    /// Gets the information about the result of executing the requested block.
    #[instrument(name = "get_state_update", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn get_state_update<B>(&self, block_id: B) -> Result<MaybePreConfirmedStateUpdate, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_state_update(block_id))
    }

    /// Gets the value of the storage at the given address and key.
    #[instrument(name = "get_storage_at", skip(self, contract_address, key, block_id), fields(contract_address = ?contract_address.as_ref(), key = ?key.as_ref(), block_id = ?block_id.as_ref()))]
    async fn get_storage_at<A, K, B>(&self, contract_address: A, key: K, block_id: B) -> Result<Felt, ProviderError>
    where
        A: AsRef<Felt> + Send + Sync,
        K: AsRef<Felt> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_storage_at(contract_address, key, block_id))
    }

    #[instrument(name = "get_messages_status", skip(self))]
    async fn get_messages_status(&self, transaction_hash: Hash256) -> Result<Vec<MessageStatus>, ProviderError> {
        call_with_fallback!(self.get_messages_status(transaction_hash))
    }

    /// Gets the transaction status (possibly reflecting that the tx is still in the mempool, or
    /// dropped from it).
    #[instrument(name = "get_transaction_status", skip(self, transaction_hash), fields(transaction_hash = ?transaction_hash.as_ref()))]
    async fn get_transaction_status<H>(&self, transaction_hash: H) -> Result<TransactionStatus, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_transaction_status(transaction_hash))
    }

    /// Gets the details and status of a submitted transaction.
    #[instrument(name = "get_transaction_by_hash", skip(self, transaction_hash), fields(transaction_hash = ?transaction_hash.as_ref()))]
    async fn get_transaction_by_hash<H>(&self, transaction_hash: H) -> Result<Transaction, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_transaction_by_hash(transaction_hash))
    }

    /// Gets the details of a transaction by a given block id and index.
    #[instrument(name = "get_transaction_by_block_id_and_index", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn get_transaction_by_block_id_and_index<B>(&self, block_id: B, index: u64) -> Result<Transaction, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_transaction_by_block_id_and_index(block_id, index))
    }

    /// Gets the details of a transaction by a given block number and index.
    #[instrument(name = "get_transaction_receipt", skip(self, transaction_hash), fields(transaction_hash = ?transaction_hash.as_ref()))]
    async fn get_transaction_receipt<H>(&self, transaction_hash: H) -> Result<TransactionReceiptWithBlockInfo, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_transaction_receipt(transaction_hash))
    }

    /// Gets the contract class definition in the given block associated with the given hash.
    #[instrument(name = "get_class", skip(self, block_id, class_hash), fields(block_id = ?block_id.as_ref(), class_hash = ?class_hash.as_ref()))]
    async fn get_class<B, H>(&self, block_id: B, class_hash: H) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        H: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_class(block_id, class_hash))
    }

    /// Gets the contract class hash in the given block for the contract deployed at the given
    /// address.
    #[instrument(name = "get_class_hash_at", skip(self, block_id, contract_address), fields(block_id = ?block_id.as_ref(), contract_address = ?contract_address.as_ref()))]
    async fn get_class_hash_at<B, A>(&self, block_id: B, contract_address: A) -> Result<Felt, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_class_hash_at(block_id, contract_address))
    }

    /// Gets the contract class definition in the given block at the given address.
    #[instrument(name = "get_class_at", skip(self, block_id, contract_address), fields(block_id = ?block_id.as_ref(), contract_address = ?contract_address.as_ref()))]
    async fn get_class_at<B, A>(&self, block_id: B, contract_address: A) -> Result<ContractClass, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_class_at(block_id, contract_address))
    }

    /// Gets the number of transactions in a block given a block id.
    #[instrument(name = "get_block_transaction_count", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn get_block_transaction_count<B>(&self, block_id: B) -> Result<u64, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.get_block_transaction_count(block_id))
    }

    /// Calls a starknet function without creating a Starknet transaction.
    #[instrument(name = "call", skip(self, request), fields(request = ?request.as_ref(), block_id = ?block_id.as_ref()))]
    async fn call<R, B>(&self, request: R, block_id: B) -> Result<Vec<Felt>, ProviderError>
    where
        R: AsRef<FunctionCall> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.call(request, block_id))
    }

    /// Estimates the fee for a given Starknet transaction.
    #[instrument(name = "estimate_fee", skip(self, request, simulation_flags, block_id), fields(request = ?request.as_ref(), simulation_flags = ?simulation_flags.as_ref(), block_id = ?block_id.as_ref()))]
    async fn estimate_fee<R, S, B>(&self, request: R, simulation_flags: S, block_id: B) -> Result<Vec<FeeEstimate>, ProviderError>
    where
        R: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlagForEstimateFee]> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.estimate_fee(request, simulation_flags, block_id))
    }

    /// Estimates the fee for sending an L1-to-L2 message.
    #[instrument(name = "estimate_message_fee", skip(self, message, block_id), fields(message = ?message.as_ref(), block_id = ?block_id.as_ref()))]
    async fn estimate_message_fee<M, B>(&self, message: M, block_id: B) -> Result<MessageFeeEstimate, ProviderError>
    where
        M: AsRef<MsgFromL1> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.estimate_message_fee(message, block_id))
    }

    /// Gets the most recent accepted block number.
    #[instrument(name = "block_number", skip(self))]
    async fn block_number(&self) -> Result<u64, ProviderError> {
        call_with_fallback!(self.block_number())
    }

    /// Gets the most recent accepted block hash and number.
    #[instrument(name = "block_hash_and_number", skip(self))]
    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, ProviderError> {
        call_with_fallback!(self.block_hash_and_number())
    }

    /// Returns the currently configured Starknet chain id.
    #[instrument(name = "chain_id", skip(self))]
    async fn chain_id(&self) -> Result<Felt, ProviderError> {
        call_with_fallback!(self.chain_id())
    }

    /// Returns an object about the sync status, or false if the node is not synching.
    #[instrument(name = "syncing", skip(self))]
    async fn syncing(&self) -> Result<SyncStatusType, ProviderError> {
        call_with_fallback!(self.syncing())
    }

    /// Returns all events matching the given filter.
    #[instrument(name = "get_events", skip(self))]
    async fn get_events(&self, filter: EventFilter, continuation_token: Option<String>, chunk_size: u64) -> Result<EventsPage, ProviderError> {
        call_with_fallback!(self.get_events(filter, continuation_token, chunk_size))
    }

    /// Gets the nonce associated with the given address in the given block.
    #[instrument(name = "get_nonce", skip(self, block_id, contract_address), fields(block_id = ?block_id.as_ref(), contract_address = ?contract_address.as_ref()))]
    async fn get_nonce<B, A>(&self, block_id: B, contract_address: A) -> Result<Felt, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        A: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.get_nonce(block_id, contract_address))
    }

    #[instrument(name = "get_storage_proof", skip(self, block_id, class_hashes, contract_addresses, contracts_storage_keys), fields(block_id = ?block_id.as_ref(), class_hashes = ?class_hashes.as_ref(), contract_addresses = ?contract_addresses.as_ref(), contracts_storage_keys = ?contracts_storage_keys.as_ref()))]
    async fn get_storage_proof<B, H, A, K>(&self, block_id: B, class_hashes: H, contract_addresses: A, contracts_storage_keys: K) -> Result<StorageProof, ProviderError>
    where
        B: AsRef<ConfirmedBlockId> + Send + Sync,
        H: AsRef<[Felt]> + Send + Sync,
        A: AsRef<[Felt]> + Send + Sync,
        K: AsRef<[ContractStorageKeys]> + Send + Sync,
    {
        call_with_fallback!(self.get_storage_proof(block_id, class_hashes, contract_addresses, contracts_storage_keys))
    }

    /// Submits a new transaction to be added to the chain.
    #[instrument(name = "add_invoke_transaction", skip(self, invoke_transaction), fields(invoke_transaction = ?invoke_transaction.as_ref()))]
    async fn add_invoke_transaction<I>(&self, invoke_transaction: I) -> Result<InvokeTransactionResult, ProviderError>
    where
        I: AsRef<BroadcastedInvokeTransaction> + Send + Sync,
    {
        call_with_fallback!(self.add_invoke_transaction(invoke_transaction))
    }

    /// Submits a new transaction to be added to the chain.
    #[instrument(name = "add_declare_transaction", skip(self, declare_transaction), fields(declare_transaction = ?declare_transaction.as_ref()))]
    async fn add_declare_transaction<D>(&self, declare_transaction: D) -> Result<DeclareTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeclareTransaction> + Send + Sync,
    {
        call_with_fallback!(self.add_declare_transaction(declare_transaction))
    }

    /// Submits a new deploy account transaction.
    #[instrument(name = "add_deploy_account_transaction", skip(self, deploy_account_transaction), fields(deploy_account_transaction = ?deploy_account_transaction.as_ref()))]
    async fn add_deploy_account_transaction<D>(&self, deploy_account_transaction: D) -> Result<DeployAccountTransactionResult, ProviderError>
    where
        D: AsRef<BroadcastedDeployAccountTransaction> + Send + Sync,
    {
        call_with_fallback!(self.add_deploy_account_transaction(deploy_account_transaction))
    }

    /// For a given executed transaction, returns the trace of its execution, including internal
    /// calls.
    #[instrument(name = "trace_transaction", skip(self, transaction_hash), fields(transaction_hash = ?transaction_hash.as_ref()))]
    async fn trace_transaction<H>(&self, transaction_hash: H) -> Result<TransactionTrace, ProviderError>
    where
        H: AsRef<Felt> + Send + Sync,
    {
        call_with_fallback!(self.trace_transaction(transaction_hash))
    }

    /// Simulates a given sequence of transactions on the requested state, and generate the
    /// execution traces. Note that some of the transactions may revert, in which case no error is
    /// thrown, but revert details can be seen on the returned trace object.
    ///
    /// Note that some of the transactions may revert, this will be reflected by the `revert_error`
    /// property in the trace. Other types of failures (e.g. unexpected error or failure in the
    /// validation phase) will result in `TRANSACTION_EXECUTION_ERROR`.
    #[instrument(name = "simulate_transactions", skip(self, block_id, transactions, simulation_flags), fields(block_id = ?block_id.as_ref(), transactions = ?transactions.as_ref(), simulation_flags = ?simulation_flags.as_ref()))]
    async fn simulate_transactions<B, T, S>(&self, block_id: B, transactions: T, simulation_flags: S) -> Result<Vec<SimulatedTransaction>, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        T: AsRef<[BroadcastedTransaction]> + Send + Sync,
        S: AsRef<[SimulationFlag]> + Send + Sync,
    {
        call_with_fallback!(self.simulate_transactions(block_id, transactions, simulation_flags))
    }

    /// Retrieves traces for all transactions in the given block.
    #[instrument(name = "trace_block_transactions", skip(self, block_id), fields(block_id = ?block_id.as_ref()))]
    async fn trace_block_transactions<B>(&self, block_id: B) -> Result<Vec<TransactionTraceWithHash>, ProviderError>
    where
        B: AsRef<ConfirmedBlockId> + Send + Sync,
    {
        call_with_fallback!(self.trace_block_transactions(block_id))
    }

    /// Sends multiple requests in parallel. The function call fails if any of the requests fails.
    /// Implementations must guarantee that responses follow the exact order as the requests.
    #[instrument(name = "batch_requests", skip(self, requests), fields(requests = ?requests.as_ref()))]
    async fn batch_requests<R>(&self, requests: R) -> Result<Vec<ProviderResponseData>, ProviderError>
    where
        R: AsRef<[ProviderRequestData]> + Send + Sync,
    {
        call_with_fallback!(self.batch_requests(requests))
    }

    #[instrument(name = "estimate_fee_single", skip(self, block_id, request, simulation_flags), fields(block_id = ?block_id.as_ref(), request = ?request.as_ref(), simulation_flags = ?simulation_flags.as_ref()))]
    async fn estimate_fee_single<R, S, B>(&self, request: R, simulation_flags: S, block_id: B) -> Result<FeeEstimate, ProviderError>
    where
        R: AsRef<BroadcastedTransaction> + Send + Sync,
        S: AsRef<[SimulationFlagForEstimateFee]> + Send + Sync,
        B: AsRef<BlockId> + Send + Sync,
    {
        call_with_fallback!(self.estimate_fee_single(request, simulation_flags, block_id))
    }

    #[instrument(name = "simulate_transaction", skip(self, block_id, transaction, simulation_flags), fields(block_id = ?block_id.as_ref(), transaction = ?transaction.as_ref(), simulation_flags = ?simulation_flags.as_ref()))]
    async fn simulate_transaction<B, T, S>(&self, block_id: B, transaction: T, simulation_flags: S) -> Result<SimulatedTransaction, ProviderError>
    where
        B: AsRef<BlockId> + Send + Sync,
        T: AsRef<BroadcastedTransaction> + Send + Sync,
        S: AsRef<[SimulationFlag]> + Send + Sync,
    {
        call_with_fallback!(self.simulate_transaction(block_id, transaction, simulation_flags))
    }
}
