use std::sync::Arc;

use ethers::prelude::{Address, U256};
use ethers::types::{
    FeeHistory, Filter, Log, SyncProgress, SyncingStatus, Transaction, TransactionReceipt, H256,
};
use eyre::{eyre, Result};

use common::errors::BlockNotFoundError;
use common::types::{Block, BlockTag};
use config::Config;

use consensus::rpc::nimbus_rpc::NimbusRpc;
use consensus::ConsensusClient;
// use execution::evm::Evm;
use execution::rpc::http_rpc::HttpRpc;
use execution::types::CallOpts;
use execution::ExecutionClient;

use crate::errors::NodeError;

pub struct Node {
    pub execution: Arc<ExecutionClient<HttpRpc>>,
    pub config: Arc<Config>,
    pub history_size: usize,
}

impl Node {
    pub fn new(config: Arc<Config>) -> Result<Self, NodeError> {
        let consensus_rpc = &config.consensus_rpc;
        let checkpoint_hash = &config.checkpoint.as_ref().unwrap();
        let execution_rpc = &config.execution_rpc;

        let consensus =
            ConsensusClient::<NimbusRpc>::new(consensus_rpc, checkpoint_hash, config.clone())
                .map_err(NodeError::ConsensusClientCreationError)?;

        let execution = Arc::new(
            ExecutionClient::new(
                execution_rpc,
                consensus.block_recv,
                consensus.finalized_block_recv,
            )
            .map_err(NodeError::ExecutionClientCreationError)?,
        );

        Ok(Node {
            execution,
            config,
            history_size: 64,
        })
    }

    // pub async fn call(&self, opts: &CallOpts, block: BlockTag) -> Result<Vec<u8>, NodeError> {
    //     // self.update_paylods();
    //     self.check_blocktag_age(&block)?;

    //     let payload = self.get_payload(block)?;
    //     let mut evm = Evm::new(
    //         self.execution.clone(),
    //         payload,
    //         &self.payloads,
    //         self.chain_id(),
    //     );
    //     evm.call(opts).await.map_err(NodeError::ExecutionEvmError)
    // }

    // pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64, NodeError> {
    //     self.check_head_age()?;

    //     let payload = self.get_payload(BlockTag::Latest)?;
    //     let mut evm = Evm::new(
    //         self.execution.clone(),
    //         payload,
    //         &self.payloads,
    //         self.chain_id(),
    //     );
    //     evm.estimate_gas(opts)
    //         .await
    //         .map_err(NodeError::ExecutionEvmError)
    // }

    pub async fn get_block(&self, tag: BlockTag) -> Result<Block<Transaction>> {
        self.execution.get_block(tag).await
    }

    pub async fn get_balance(&self, address: &Address, block: BlockTag) -> Result<U256> {
        self.check_blocktag_age(&block)?;
        let account = self.execution.get_account(address, None, block).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&self, address: &Address, block: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&block)?;
        let account = self.execution.get_account(address, None, block).await?;
        Ok(account.nonce)
    }

    // pub fn get_block_transaction_count_by_hash(&self, hash: &Vec<u8>) -> Result<u64> {
    //     let payload = self.get_payload_by_hash(hash)?;
    //     let transaction_count = payload.1.transactions().len();

    //     Ok(transaction_count as u64)
    // }

    pub async fn get_block_transaction_count_by_number(&self, block: BlockTag) -> Result<u64> {
        Ok(self.execution.get_block(block).await?.transactions.len() as u64)
    }

    pub async fn get_code(&self, address: &Address, block: BlockTag) -> Result<Vec<u8>> {
        self.check_blocktag_age(&block)?;
        let account = self.execution.get_account(address, None, block).await?;
        Ok(account.code)
    }

    pub async fn get_storage_at(
        &self,
        address: &Address,
        slot: H256,
        block: BlockTag,
    ) -> Result<U256> {
        self.check_head_age()?;

        let account = self
            .execution
            .get_account(address, Some(&[slot]), block)
            .await?;

        let value = account.slots.get(&slot);
        match value {
            Some(value) => Ok(*value),
            None => Err(eyre!("slot not found")),
        }
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256> {
        self.execution.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        self.execution.get_transaction_receipt(tx_hash).await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        self.execution.get_transaction(tx_hash).await
    }

    // pub async fn get_transaction_by_block_hash_and_index(
    //     &self,
    //     hash: &Vec<u8>,
    //     index: usize,
    // ) -> Result<Option<Transaction>> {
    //     self.execution.get_transaction_by_block_hash_and_index(block, index).await
    // }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter).await
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub async fn get_gas_price(&self) -> Result<U256> {
        self.check_head_age()?;

        let base_fee = self
            .execution
            .get_block(BlockTag::Latest)
            .await?
            .base_fee_per_gas;
        let tip = U256::from(10_u64.pow(9));

        Ok(base_fee + tip)
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub fn get_priority_fee(&self) -> Result<U256> {
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        self.check_head_age()?;

        let payload = self.execution.get_block(BlockTag::Latest).await?;
        Ok(payload.number.as_u64())
    }

    // pub async fn get_block_by_number(
    //     &self,
    //     block: BlockTag,
    //     full_tx: bool,
    // ) -> Result<Option<ExecutionBlock>> {
    //     self.check_blocktag_age(&block)?;

    //     match self.execution.get_block(block) {
    //         Ok(block) => {
    //             if full_tx {

    //             }
    //         },
    //         Err(_) => Ok(None),
    //     }
    // }

    // pub async fn get_fee_history(
    //     &self,
    //     block_count: u64,
    //     last_block: u64,
    //     reward_percentiles: &[f64],
    // ) -> Result<Option<FeeHistory>> {
    //     self.execution
    //         .get_fee_history(block_count, last_block, reward_percentiles, &self.payloads)
    //         .await
    // }

    // pub async fn get_block_by_hash(
    //     &self,
    //     hash: &Vec<u8>,
    //     full_tx: bool,
    // ) -> Result<Option<ExecutionBlock>> {
    //     let payload = self.get_payload_by_hash(hash);

    //     match payload {
    //         Ok(payload) => self.execution.get_block(payload.1, full_tx).await.map(Some),
    //         Err(_) => Ok(None),
    //     }
    // }

    pub fn chain_id(&self) -> u64 {
        self.config.chain.chain_id
    }

    pub async fn syncing(&self) -> Result<SyncingStatus> {
        if self.check_head_age().is_ok() {
            Ok(SyncingStatus::IsFalse)
        } else {
            let latest_synced_block = self.get_block_number().await?;

            // let highest_block = self.consensus.expected_current_slot();

            Ok(SyncingStatus::IsSyncing(Box::new(SyncProgress {
                current_block: latest_synced_block.into(),
                // TODO: fixmme
                highest_block: 0.into(),
                starting_block: 0.into(),
                pulled_states: None,
                known_states: None,
                healed_bytecode_bytes: None,
                healed_bytecodes: None,
                healed_trienode_bytes: None,
                healed_trienodes: None,
                healing_bytecode: None,
                healing_trienodes: None,
                synced_account_bytes: None,
                synced_accounts: None,
                synced_bytecode_bytes: None,
                synced_bytecodes: None,
                synced_storage: None,
                synced_storage_bytes: None,
            })))
        }
    }

    // pub fn get_header(&self) -> Result<Header> {
    //     self.check_head_age()?;
    //     Ok(self.consensus.get_header().clone())
    // }

    pub async fn get_coinbase(&self) -> Result<Address> {
        self.check_head_age()?;
        let block = self.execution.get_block(BlockTag::Latest).await?;
        Ok(block.miner)
    }

    pub fn get_last_checkpoint(&self) -> Option<Vec<u8>> {
        // self.consensus.last_checkpoint.clone()
        None
    }

    // fn get_payload(&self, block: BlockTag) -> Result<&ExecutionPayload, BlockNotFoundError> {
    //     match block {
    //         BlockTag::Latest => {
    //             let payload = self.payloads.last_key_value();
    //             Ok(payload.ok_or(BlockNotFoundError::new(BlockTag::Latest))?.1)
    //         }
    //         BlockTag::Finalized => {
    //             let payload = self.finalized_payloads.last_key_value();
    //             Ok(payload
    //                 .ok_or(BlockNotFoundError::new(BlockTag::Finalized))?
    //                 .1)
    //         }
    //         BlockTag::Number(num) => {
    //             let payload = self.payloads.get(&num);
    //             payload.ok_or(BlockNotFoundError::new(BlockTag::Number(num)))
    //         }
    //     }
    // }

    // fn get_payload_by_hash(&self, hash: &Vec<u8>) -> Result<(&u64, &ExecutionPayload)> {
    //     let payloads = self
    //         .payloads
    //         .iter()
    //         .filter(|entry| &entry.1.block_hash().to_vec() == hash)
    //         .collect::<Vec<(&u64, &ExecutionPayload)>>();

    //     payloads
    //         .get(0)
    //         .cloned()
    //         .ok_or(eyre!("Block not found by hash"))
    // }

    fn check_head_age(&self) -> Result<(), NodeError> {
        // let synced_slot = self.consensus.get_header().slot.as_u64();
        // let expected_slot = self.consensus.expected_current_slot();
        // let slot_delay = expected_slot - synced_slot;

        // if slot_delay > 10 {
        //     return Err(NodeError::OutOfSync(slot_delay));
        // }

        Ok(())
    }

    fn check_blocktag_age(&self, block: &BlockTag) -> Result<(), NodeError> {
        match block {
            BlockTag::Latest => self.check_head_age(),
            BlockTag::Finalized => Ok(()),
            BlockTag::Number(_) => Ok(()),
        }
    }
}
