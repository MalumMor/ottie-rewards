// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

use crate::errors::RewardError;
use crate::events::DistributionClosed;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
#[instruction(period: u32)]
pub struct CloseDistribution<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, RewardConfig>,
    #[account(
        mut,
        seeds = [DISTRIBUTION_SEED, &period.to_le_bytes()],
        bump = distribution.bump
    )]
    pub distribution: Account<'info, Distribution>,
}

pub fn process_close_distribution(ctx: Context<CloseDistribution>, period: u32) -> Result<()> {
    let config = &ctx.accounts.config;
    let distribution = &ctx.accounts.distribution;
    let authority = ctx.accounts.authority.key();

    match config.open_period {
        None => return err!(RewardError::NoOpenDistribution),
        Some(open) => require!(open == period && distribution.period == period, RewardError::WrongPeriod),
    }
    if authority == config.admin {
    } else if authority == config.distributor {
        require!(!config.paused, RewardError::Paused);
        require!(
            distribution.minted_count == distribution.recipient_count,
            RewardError::DistributionNotFullyPaid
        );
    } else {
        return err!(RewardError::InvalidAuthority);
    }

    let now = Clock::get()?.unix_timestamp;
    let unminted = distribution
        .recipient_count
        .checked_sub(distribution.minted_count)
        .ok_or(RewardError::MathOverflow)?;
    let minted_total = distribution.minted_total;
    let minted_count = distribution.minted_count;

    ctx.accounts.distribution.closed_at = now;
    ctx.accounts.config.open_period = None;

    emit_cpi!(DistributionClosed { period, minted_total, minted_count, unminted, by: authority });
    Ok(())
}
