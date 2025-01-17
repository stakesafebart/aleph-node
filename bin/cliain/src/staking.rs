use aleph_client::{
    staking_bond, staking_force_new_era, staking_set_staking_limits, staking_validate, Connection,
};
use primitives::TOKEN;
use sp_core::crypto::Ss58Codec;
use substrate_api_client::{AccountId, XtStatus};

pub fn bond(
    stash_connection: Connection,
    initial_stake_in_tokens: u32,
    controller_account: String,
) {
    let controller_account =
        AccountId::from_ss58check(&controller_account).expect("Address is valid");

    let initial_stake = initial_stake_in_tokens as u128 * TOKEN;
    staking_bond(
        &stash_connection,
        initial_stake,
        &controller_account,
        XtStatus::Finalized,
    );
}

pub fn validate(connection: Connection, commission_percentage: u8) {
    staking_validate(&connection, commission_percentage, XtStatus::Finalized);
}

pub fn set_staking_limits(
    root_connection: Connection,
    minimal_nominator_stake_tokens: u64,
    minimal_validator_stake_tokens: u64,
) {
    staking_set_staking_limits(
        &root_connection,
        minimal_nominator_stake_tokens as u128 * TOKEN,
        minimal_validator_stake_tokens as u128 * TOKEN,
        XtStatus::Finalized,
    );
}

pub fn force_new_era(root_connection: Connection) {
    staking_force_new_era(&root_connection, XtStatus::Finalized);
}
