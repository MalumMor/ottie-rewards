// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

use crate::errors::RewardError;
use crate::events::LimitsChanged;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetLimits<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RewardError::NotAdmin
    )]
    pub config: Account<'info, RewardConfig>,
}

pub fn process_set_limits(ctx: Context<SetLimits>, limits: Limits) -> Result<()> {
    let config = &mut ctx.accounts.config;
    validate_limits(&limits, config.max_supply)?;
    config.max_amount_per_distribution = limits.max_amount_per_distribution;
    config.max_recipients_per_distribution = limits.max_recipients_per_distribution;
    config.max_amount_per_recipient = limits.max_amount_per_recipient;
    config.min_seconds_between_openings = limits.min_seconds_between_openings;
    config.mint_delay_seconds = limits.mint_delay_seconds;

    emit_cpi!(LimitsChanged {
        max_amount_per_distribution: limits.max_amount_per_distribution,
        max_recipients_per_distribution: limits.max_recipients_per_distribution,
        max_amount_per_recipient: limits.max_amount_per_recipient,
        min_seconds_between_openings: limits.min_seconds_between_openings,
        mint_delay_seconds: limits.mint_delay_seconds,
    });
    Ok(())
}
