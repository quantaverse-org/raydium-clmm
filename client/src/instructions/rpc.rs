use anyhow::{anyhow, Result};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::{
    api::{
        config::{RpcProgramAccountsConfig, RpcSendTransactionConfig},
        request::RpcRequest,
        response::{RpcResult, RpcSimulateTransactionResult},
    },
    rpc_client::RpcClient,
};
use solana_sdk::{
    account::Account, program_pack::Pack as TokenPack, pubkey::Pubkey, signature::Signature,
    transaction::Transaction,
};
use std::convert::Into;

pub fn simulate_transaction(
    client: &RpcClient,
    transaction: &Transaction,
    sig_verify: bool,
    cfg: CommitmentConfig,
) -> RpcResult<RpcSimulateTransactionResult> {
    let serialized_encoded = bs58::encode(bincode::serialize(transaction).unwrap()).into_string();
    client.send(
        RpcRequest::SimulateTransaction,
        serde_json::json!([serialized_encoded, {
            "sigVerify": sig_verify, "commitment": cfg.commitment
        }]),
    )
}

pub fn send_txn(client: &RpcClient, txn: &Transaction, wait_confirm: bool) -> Result<Signature> {
    Ok(client.send_and_confirm_transaction_with_spinner_and_config(
        txn,
        if wait_confirm {
            CommitmentConfig::confirmed()
        } else {
            CommitmentConfig::processed()
        },
        RpcSendTransactionConfig {
            skip_preflight: true,
            ..RpcSendTransactionConfig::default()
        },
    )?)
}

pub fn get_token_account<T: TokenPack>(client: &RpcClient, addr: &Pubkey) -> Result<T> {
    let account = client
        .get_account_with_commitment(addr, CommitmentConfig::processed())?
        .value
        .map_or(Err(anyhow!("Account not found")), Ok)?;
    T::unpack_from_slice(&account.data).map_err(Into::into)
}

pub fn get_multiple_accounts(
    client: &RpcClient,
    pubkeys: &[Pubkey],
) -> Result<Vec<Option<Account>>> {
    Ok(client.get_multiple_accounts(pubkeys)?)
}

pub fn get_program_accounts_with_config(
    client: &RpcClient,
    program_id: &Pubkey,
    config: RpcProgramAccountsConfig,
) -> Result<Vec<(Pubkey, Account)>> {
    client
        .get_program_ui_accounts_with_config(program_id, config)?
        .into_iter()
        .map(|(pubkey, account)| {
            let account = account
                .to_account()
                .ok_or_else(|| anyhow!("Failed to decode account {pubkey}"))?;
            Ok((pubkey, account))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_account_decoder::{encode_ui_account, UiAccountEncoding};
    use solana_rpc_client::api::response::RpcKeyedAccount;

    #[test]
    fn decodes_program_accounts_from_rpc() {
        let pubkey = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();
        let account = Account {
            owner: program_id,
            data: vec![1, 2, 3],
            ..Account::default()
        };
        let keyed_account = RpcKeyedAccount {
            pubkey: pubkey.to_string(),
            account: encode_ui_account(
                &pubkey,
                &account,
                UiAccountEncoding::Base64Zstd,
                None,
                None,
            ),
        };
        let mocks = [(
            RpcRequest::GetProgramAccounts,
            serde_json::json!([keyed_account]),
        )]
        .into();
        let client = RpcClient::new_mock_with_mocks("succeeds", mocks);
        let accounts = get_program_accounts_with_config(
            &client,
            &program_id,
            RpcProgramAccountsConfig::default(),
        )
        .unwrap();

        assert_eq!(accounts, vec![(pubkey, account)]);
    }
}
