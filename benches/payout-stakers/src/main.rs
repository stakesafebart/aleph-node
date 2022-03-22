use aleph_client::{create_connection, staking_bond, staking_validate, Protocol};
use e2e::{
    accounts::derive_user_account,
    staking::{
        batch_bond, batch_nominate, check_non_zero_payouts_for_era, wait_for_era_completion,
        wait_for_full_era_completion,
    },
    transfer::batch_endow_account_balances,
};
use log::info;
use primitives::staking::{MIN_NOMINATOR_BOND, MIN_VALIDATOR_BOND};
use rayon::prelude::*;
use sp_core::{sr25519::Pair as KeyPair, Pair};
use sp_keyring::AccountKeyring;
use std::iter;
use substrate_api_client::{
    extrinsic::staking::RewardDestination,
    {rpc::WsRpcClient, AccountId, Api, XtStatus},
};

// testcase parameters
const NOMINATOR_COUNT: u64 = 1024;
const VALIDATOR_COUNT: u64 = 4;
const ERAS_TO_WAIT: u64 = 10;

// we need to schedule batches for limited call count, otherwise we'll exhaust a block max weight
const BOND_CALL_BATCH_LIMIT: usize = 256;
const NOMINATE_CALL_BATCH_LIMIT: usize = 128;

fn main() -> Result<(), anyhow::Error> {
    let address = "127.0.0.1:9944";
    let protocol = Protocol::WS;
    let sudoer = AccountKeyring::Alice.pair();

    env_logger::init();
    info!("Starting benchmark with config ");

    let connection = create_connection(address, protocol).set_signer(sudoer);

    let accounts = generate_1024_accounts(&connection);
    let validators = set_validators(address, protocol);
    let nominee = nominate_validator_0(&connection, accounts, &validators);
    wait_for_10_eras(address, protocol, &connection, nominee)?;

    Ok(())
}

fn wait_for_10_eras(
    address: &str,
    protocol: Protocol,
    connection: &Api<KeyPair, WsRpcClient>,
    nominee: &KeyPair,
) -> Result<(), anyhow::Error> {
    let mut current_era = wait_for_full_era_completion(&connection)?;
    for _ in 0..ERAS_TO_WAIT {
        info!(
            "Era {} started, claiming rewards for era {}",
            current_era,
            current_era - 1
        );
        let stash_connection = create_connection(address, protocol).set_signer(nominee.clone());
        check_non_zero_payouts_for_era(&stash_connection, nominee, &connection, current_era);
        current_era = wait_for_era_completion(&connection, current_era + 1)?;
    }
    Ok(())
}

fn nominate_validator_0<'a>(
    connection: &Api<KeyPair, WsRpcClient>,
    accounts: Vec<KeyPair>,
    validators: &'a Vec<KeyPair>,
) -> &'a KeyPair {
    // 3. Let accounts nominate validator[0]
    let nominee = &validators[0];
    let stash_validators_pairs = accounts.iter().zip(accounts.iter()).collect::<Vec<_>>();
    stash_validators_pairs
        .chunks(BOND_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            batch_bond(
                &connection,
                chunk,
                MIN_NOMINATOR_BOND,
                RewardDestination::Staked,
            )
        });
    let nominator_nominee_pairs = accounts
        .iter()
        .zip(iter::repeat(nominee))
        .collect::<Vec<_>>();
    nominator_nominee_pairs
        .chunks(NOMINATE_CALL_BATCH_LIMIT)
        .for_each(|chunk| batch_nominate(&connection, chunk));
    nominee
}

fn set_validators(address: &str, protocol: Protocol) -> Vec<KeyPair> {
    let validators = (0..VALIDATOR_COUNT)
        .map(derive_user_account)
        .collect::<Vec<_>>();
    validators.par_iter().for_each(|account| {
        let connection = create_connection(address, protocol).set_signer(account.clone());
        let controller_account_id = AccountId::from(account.public());
        staking_bond(
            &connection,
            MIN_VALIDATOR_BOND,
            &controller_account_id,
            XtStatus::InBlock,
        );
    });
    validators.par_iter().for_each(|account| {
        let connection = create_connection(address, protocol).set_signer(account.clone());
        staking_validate(&connection, 10, XtStatus::InBlock);
    });
    validators
}

fn generate_1024_accounts(connection: &Api<KeyPair, WsRpcClient>) -> Vec<KeyPair> {
    let accounts = (VALIDATOR_COUNT..NOMINATOR_COUNT + VALIDATOR_COUNT)
        .map(derive_user_account)
        .collect::<Vec<_>>();
    batch_endow_account_balances(&connection, &accounts, MIN_NOMINATOR_BOND);
    accounts
}
