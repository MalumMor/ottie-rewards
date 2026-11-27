// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

use crate::errors::RewardError;
use crate::events::PauseChanged;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetPaused<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, RewardConfig>,
}

pub fn process_set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    let authority = ctx.accounts.authority.key();
    let config = &mut ctx.accounts.config;
    if authority == config.admin {
        if !paused {
            config.genesis_closed = true;
        }
    } else if authority == config.distributor {
        require!(paused, RewardError::NotAdmin);
    } else {
        return err!(RewardError::InvalidAuthority);
    }
    config.paused = paused;

    emit_cpi!(PauseChanged { paused, by: authority });
    Ok(())
}
