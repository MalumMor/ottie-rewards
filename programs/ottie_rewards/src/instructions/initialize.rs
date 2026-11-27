// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::Mint;

use crate::errors::RewardError;
use crate::events::Initialized;
use crate::program::OttieRewards;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeArgs {
    pub admin: Pubkey,
    pub distributor: Pubkey,
    pub max_supply: u64,
    pub decimals: u8,
    pub limits: Limits,
}

#[event_cpi]
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub upgrade_authority: Signer<'info>,
    #[doc = "CHECK: created by this instruction at its derived address"]
    #[account(mut, seeds = [CONFIG_SEED], bump)]
    pub config: UncheckedAccount<'info>,
    pub mint: Account<'info, Mint>,
    #[doc = "CHECK: derived signer address, holds no data"]
    #[account(seeds = [MINT_AUTHORITY_SEED], bump)]
    pub mint_authority: UncheckedAccount<'info>,
    #[account(
        constraint = this_program.programdata_address()? == Some(program_data.key()) @ RewardError::InvalidProgramData
    )]
    pub this_program: Program<'info, OttieRewards>,
    #[account(
        constraint = program_data.upgrade_authority_address == Some(upgrade_authority.key()) @ RewardError::InvalidAuthority
    )]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

pub fn process_initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
    require!(
        is_unused_account(&ctx.accounts.config.to_account_info()),
        RewardError::AlreadyInitialized
    );
    require!(
        args.admin != Pubkey::default()
            && args.distributor != Pubkey::default()
            && args.admin != args.distributor,
        RewardError::InvalidAuthority
    );
    require!(args.max_supply > 0 && args.decimals <= MAX_DECIMALS, RewardError::InvalidLimits);
    validate_limits(&args.limits, args.max_supply)?;

    let mint = &ctx.accounts.mint;
    require!(
        mint.is_initialized
            && mint.mint_authority == COption::Some(ctx.accounts.mint_authority.key())
            && mint.freeze_authority.is_none()
            && mint.supply == 0
            && mint.decimals == args.decimals,
        RewardError::InvalidMint
    );

    let config = RewardConfig {
        version: LAYOUT_VERSION,
        bump: ctx.bumps.config,
        mint_authority_bump: ctx.bumps.mint_authority,
        admin: args.admin,
        pending_admin: None,
        distributor: args.distributor,
        mint: mint.key(),
        max_supply: args.max_supply,
        genesis_minted: false,
        genesis_closed: false,
        paused: true,
        max_amount_per_distribution: args.limits.max_amount_per_distribution,
        max_recipients_per_distribution: args.limits.max_recipients_per_distribution,
        max_amount_per_recipient: args.limits.max_amount_per_recipient,
        min_seconds_between_openings: args.limits.min_seconds_between_openings,
        mint_delay_seconds: args.limits.mint_delay_seconds,
        last_period: 0,
        last_opened_at: 0,
        open_period: None,
        total_genesis: 0,
        total_reward_minted: 0,
        spend_burn_bps: BPS_DENOMINATOR,
        treasury: None,
        reserved: [0u8; 128],
    };
    create_state_account(
        &ctx.accounts.upgrade_authority.to_account_info(),
        &ctx.accounts.config.to_account_info(),
        ctx.program_id,
        RewardConfig::SPACE,
        &[CONFIG_SEED, &[ctx.bumps.config]],
        &config,
    )?;

    emit_cpi!(Initialized {
        admin: args.admin,
        distributor: args.distributor,
        mint: mint.key(),
        max_supply: args.max_supply,
        decimals: args.decimals,
    });
    Ok(())
}
