// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;
use state::Limits;

declare_id!("AnFkUo9tGqmCLe1e2nwZmGm9mA2m59TnaPhm7cEM8s4G");

#[program]
pub mod ottie_rewards {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
        instructions::initialize::process_initialize(ctx, args)
    }

    pub fn genesis_mint(ctx: Context<GenesisMint>, amount: u64) -> Result<()> {
        instructions::genesis_mint::process_genesis_mint(ctx, amount)
    }

    pub fn open_distribution(
        ctx: Context<OpenDistribution>,
        period: u32,
        recipient_count: u16,
        allocated_total: u64,
        recipients_hash: [u8; 32],
    ) -> Result<()> {
        instructions::open_distribution::process_open_distribution(ctx, period, recipient_count, allocated_total, recipients_hash)
    }

    pub fn mint_rewards<'info>(
        ctx: Context<'info, MintRewards<'info>>,
        period: u32,
        amounts: Vec<u64>,
        tail_link: [u8; 32],
    ) -> Result<()> {
        instructions::mint_rewards::process_mint_rewards(ctx, period, amounts, tail_link)
    }

    pub fn close_distribution(ctx: Context<CloseDistribution>, period: u32) -> Result<()> {
        instructions::close_distribution::process_close_distribution(ctx, period)
    }

    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        instructions::set_paused::process_set_paused(ctx, paused)
    }

    pub fn set_distributor(ctx: Context<SetDistributor>, new_distributor: Pubkey) -> Result<()> {
        instructions::set_distributor::process_set_distributor(ctx, new_distributor)
    }

    pub fn set_limits(ctx: Context<SetLimits>, limits: Limits) -> Result<()> {
        instructions::set_limits::process_set_limits(ctx, limits)
    }

    pub fn set_spend_policy(
        ctx: Context<SetSpendPolicy>,
        spend_burn_bps: u16,
        treasury: Option<Pubkey>,
    ) -> Result<()> {
        instructions::set_spend_policy::process_set_spend_policy(ctx, spend_burn_bps, treasury)
    }

    pub fn propose_admin(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
        instructions::propose_admin::process_propose_admin(ctx, new_admin)
    }

    pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
        instructions::accept_admin::process_accept_admin(ctx)
    }

    pub fn spend_for_game(ctx: Context<SpendForGame>, amount: u64, reference: [u8; 32]) -> Result<()> {
        instructions::spend_for_game::process_spend_for_game(ctx, amount, reference)
    }
}
