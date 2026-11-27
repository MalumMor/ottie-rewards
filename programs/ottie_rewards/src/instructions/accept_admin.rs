// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

use crate::errors::RewardError;
use crate::events::AdminChanged;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
pub struct AcceptAdmin<'info> {
    pub new_admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, RewardConfig>,
}

pub fn process_accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
    let new_admin = ctx.accounts.new_admin.key();
    let config = &mut ctx.accounts.config;
    require!(config.pending_admin == Some(new_admin), RewardError::NotPendingAdmin);
    require!(new_admin != config.distributor, RewardError::InvalidAuthority);
    let old = config.admin;
    config.admin = new_admin;
    config.pending_admin = None;

    emit_cpi!(AdminChanged { old, new: new_admin });
    Ok(())
}
