// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::errors::RewardError;
use crate::events::GenesisMinted;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
pub struct GenesisMint<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RewardError::NotAdmin,
        has_one = mint @ RewardError::WrongMint
    )]
    pub config: Account<'info, RewardConfig>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[doc = "CHECK: derived signer address, holds no data"]
    #[account(seeds = [MINT_AUTHORITY_SEED], bump = config.mint_authority_bump)]
    pub mint_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn process_genesis_mint(ctx: Context<GenesisMint>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(!config.genesis_minted, RewardError::GenesisAlreadyMinted);
    require!(
        !config.genesis_closed && config.paused && config.last_period == 0,
        RewardError::GenesisWindowClosed
    );
    require!(amount > 0, RewardError::AmountOutOfRange);
    require!(
        ctx.accounts.recipient_token_account.mint == config.mint,
        RewardError::WrongMint
    );
    let supply_after = ctx
        .accounts
        .mint
        .supply
        .checked_add(amount)
        .ok_or(RewardError::MathOverflow)?;
    require!(supply_after <= config.max_supply, RewardError::SupplyCapExceeded);

    let bump = [config.mint_authority_bump];
    let seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, &bump];
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    let config = &mut ctx.accounts.config;
    config.genesis_minted = true;
    config.genesis_closed = true;
    config.total_genesis = amount;

    ctx.accounts.mint.reload()?;
    emit_cpi!(GenesisMinted {
        token_account: ctx.accounts.recipient_token_account.key(),
        token_account_owner: ctx.accounts.recipient_token_account.owner,
        amount,
        supply_after: ctx.accounts.mint.supply,
    });
    Ok(())
}
