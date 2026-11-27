// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

use crate::errors::RewardError;
use crate::events::DistributorChanged;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetDistributor<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RewardError::NotAdmin
    )]
    pub config: Account<'info, RewardConfig>,
}

pub fn process_set_distributor(ctx: Context<SetDistributor>, new_distributor: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        new_distributor != Pubkey::default() && new_distributor != config.admin,
        RewardError::InvalidAuthority
    );
    let old = config.distributor;
    config.distributor = new_distributor;

    emit_cpi!(DistributorChanged { old, new: new_distributor });
    Ok(())
}
