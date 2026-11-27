// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

use crate::errors::RewardError;
use crate::state::*;

#[derive(Accounts)]
pub struct ProposeAdmin<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RewardError::NotAdmin
    )]
    pub config: Account<'info, RewardConfig>,
}

pub fn process_propose_admin(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        new_admin != Pubkey::default() && new_admin != config.distributor,
        RewardError::InvalidAuthority
    );
    config.pending_admin = Some(new_admin);
    Ok(())
}
