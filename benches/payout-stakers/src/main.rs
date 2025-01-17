use std::iter;

use log::info;
use rayon::prelude::*;
use sp_core::{sr25519::Pair as KeyPair, Pair};
use sp_keyring::AccountKeyring;
use substrate_api_client::{extrinsic::staking::RewardDestination, AccountId, XtStatus};

use aleph_client::{
    balances_batch_transfer, create_connection, keypair_from_string,
    payout_stakers_and_assert_locked_balance, staking_batch_bond, staking_batch_nominate,
    staking_bond, staking_validate, wait_for_full_era_completion, wait_for_next_era, Connection,
};
use primitives::staking::{
    MAX_NOMINATORS_REWARDED_PER_VALIDATOR, MIN_NOMINATOR_BOND, MIN_VALIDATOR_BOND,
};

// testcase parameters
const NOMINATOR_COUNT: u64 = MAX_NOMINATORS_REWARDED_PER_VALIDATOR as u64;
const VALIDATOR_COUNT: u64 = 4;
const ERAS_TO_WAIT: u64 = 10;

// we need to schedule batches for limited call count, otherwise we'll exhaust a block max weight
const BOND_CALL_BATCH_LIMIT: usize = 256;
const NOMINATE_CALL_BATCH_LIMIT: usize = 128;
const TRANSFER_CALL_BATCH_LIMIT: usize = 1024;

fn main() -> Result<(), anyhow::Error> {
    let address = "127.0.0.1:9944";
    let sudoer = AccountKeyring::Alice.pair();

    env_logger::init();
    info!("Starting benchmark with config ");

    let connection = create_connection(address).set_signer(sudoer);

    let nominator_accounts = generate_nominator_accounts_with_minimal_bond(&connection);
    let validators = set_validators(address);
    let nominee = &validators[0];
    nominate_validator_0(&connection, nominator_accounts.clone(), nominee);
    wait_for_10_eras(address, &connection, nominee, nominator_accounts)?;

    Ok(())
}

pub fn derive_user_account(seed: u64) -> KeyPair {
    keypair_from_string(&format!("//{}", seed))
}

fn wait_for_10_eras(
    address: &str,
    connection: &Connection,
    nominee: &KeyPair,
    nominators: Vec<AccountId>,
) -> Result<(), anyhow::Error> {
    // we wait at least two full eras plus this era, to have thousands of nominators to settle up
    // correctly
    wait_for_next_era(connection)?;
    let mut current_era = wait_for_full_era_completion(connection)?;
    for _ in 0..ERAS_TO_WAIT {
        info!(
            "Era {} started, claiming rewards for era {}",
            current_era,
            current_era - 1
        );
        let stash_connection = create_connection(address).set_signer(nominee.clone());
        let stash_account = AccountId::from(nominee.public());
        payout_stakers_and_assert_locked_balance(
            &stash_connection,
            &nominators[..],
            &stash_account,
            current_era,
        );
        current_era = wait_for_next_era(connection)?;
    }
    Ok(())
}

fn nominate_validator_0(
    connection: &Connection,
    nominator_accounts: Vec<AccountId>,
    nominee: &KeyPair,
) {
    let stash_validators_accounts = nominator_accounts
        .iter()
        .zip(nominator_accounts.iter())
        .collect::<Vec<_>>();
    stash_validators_accounts
        .chunks(BOND_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            staking_batch_bond(
                connection,
                chunk,
                MIN_NOMINATOR_BOND,
                RewardDestination::Staked,
            )
        });
    let nominee_account = AccountId::from(nominee.public());
    let nominator_nominee_accounts = nominator_accounts
        .iter()
        .zip(iter::repeat(&nominee_account))
        .collect::<Vec<_>>();
    nominator_nominee_accounts
        .chunks(NOMINATE_CALL_BATCH_LIMIT)
        .for_each(|chunk| staking_batch_nominate(connection, chunk));
}

fn set_validators(address: &str) -> Vec<KeyPair> {
    let validators = (0..VALIDATOR_COUNT)
        .map(derive_user_account)
        .collect::<Vec<_>>();
    validators.par_iter().for_each(|account| {
        let connection = create_connection(address).set_signer(account.clone());
        let controller_account_id = AccountId::from(account.public());
        staking_bond(
            &connection,
            MIN_VALIDATOR_BOND,
            &controller_account_id,
            XtStatus::InBlock,
        );
    });
    validators.par_iter().for_each(|account| {
        let connection = create_connection(address).set_signer(account.clone());
        staking_validate(&connection, 10, XtStatus::InBlock);
    });
    validators
}

fn generate_nominator_accounts_with_minimal_bond(connection: &Connection) -> Vec<AccountId> {
    let accounts = (VALIDATOR_COUNT..NOMINATOR_COUNT + VALIDATOR_COUNT)
        .map(derive_user_account)
        .map(|key_pair| AccountId::from(key_pair.public()))
        .collect::<Vec<_>>();
    accounts
        .chunks(TRANSFER_CALL_BATCH_LIMIT)
        .for_each(|chunk| {
            balances_batch_transfer(connection, chunk.to_vec(), MIN_NOMINATOR_BOND);
        });
    accounts
}
