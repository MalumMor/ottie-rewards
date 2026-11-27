// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::errors::RewardError;
use crate::events::DistributionOpened;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
#[instruction(period: u32)]
pub struct OpenDistribution<'info> {
    #[account(mut)]
    pub distributor: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = distributor @ RewardError::NotDistributor,
        has_one = mint @ RewardError::WrongMint
    )]
    pub config: Account<'info, RewardConfig>,
    pub mint: Account<'info, Mint>,
    #[doc = "CHECK: created by this instruction at its derived address"]
    #[account(mut, seeds = [DISTRIBUTION_SEED, &period.to_le_bytes()], bump)]
    pub distribution: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn process_open_distribution(
    ctx: Context<OpenDistribution>,
    period: u32,
    recipient_count: u16,
    allocated_total: u64,
    recipients_hash: [u8; 32],
) -> Result<()> {
    let config = &ctx.accounts.config;
    let now = Clock::get()?.unix_timestamp;

    require!(!config.paused, RewardError::Paused);
    require!(config.open_period.is_none(), RewardError::DistributionAlreadyOpen);
    require!(
        period == previous_period(utc_period(now)?)?,
        RewardError::PeriodNotJustClosed
    );
    require!(period > config.last_period, RewardError::PeriodNotIncreasing);
    if config.last_opened_at != 0 {
        let earliest = config
            .last_opened_at
            .checked_add(config.min_seconds_between_openings)
            .ok_or(RewardError::MathOverflow)?;
        require!(now >= earliest, RewardError::OpeningTooSoon);
    }
    require!(
        (1..=config.max_recipients_per_distribution).contains(&recipient_count),
        RewardError::RecipientCountOutOfRange
    );
    require!(
        (1..=config.max_amount_per_distribution).contains(&allocated_total),
        RewardError::AllocationOutOfRange
    );
    let per_recipient_bound = (recipient_count as u64)
        .checked_mul(config.max_amount_per_recipient)
        .ok_or(RewardError::MathOverflow)?;
    require!(
        allocated_total <= per_recipient_bound,
        RewardError::AllocationExceedsPerRecipientBound
    );
    let supply_before = ctx.accounts.mint.supply;
    let supply_committed = supply_before
        .checked_add(allocated_total)
        .ok_or(RewardError::MathOverflow)?;
    require!(supply_committed <= config.max_supply, RewardError::SupplyCapExceeded);
    require!(recipients_hash != EMPTY_LINK, RewardError::CommitmentMismatch);
    require!(
        is_unused_account(&ctx.accounts.distribution.to_account_info()),
        RewardError::PeriodNotIncreasing
    );

    let mint_not_before = now
        .checked_add(config.mint_delay_seconds)
        .ok_or(RewardError::MathOverflow)?;
    let distribution = Distribution {
        bump: ctx.bumps.distribution,
        period,
        recipient_count,
        allocated_total,
        minted_total: 0,
        minted_count: 0,
        recipients_hash,
        next_link: recipients_hash,
        opened_at: now,
        mint_not_before,
        closed_at: 0,
    };
    create_state_account(
        &ctx.accounts.distributor.to_account_info(),
        &ctx.accounts.distribution.to_account_info(),
        ctx.program_id,
        Distribution::SPACE,
        &[DISTRIBUTION_SEED, &period.to_le_bytes(), &[ctx.bumps.distribution]],
        &distribution,
    )?;

    let config = &mut ctx.accounts.config;
    config.open_period = Some(period);
    config.last_period = period;
    config.last_opened_at = now;

    emit_cpi!(DistributionOpened {
        period,
        recipient_count,
        allocated_total,
        recipients_hash,
        mint_not_before,
        supply_before,
    });
    Ok(())
}
