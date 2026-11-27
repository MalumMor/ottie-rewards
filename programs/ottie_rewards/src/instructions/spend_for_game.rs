// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};

use crate::errors::RewardError;
use crate::events::GameSpend;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
#[instruction(amount: u64, reference: [u8; 32])]
pub struct SpendForGame<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = mint @ RewardError::WrongMint
    )]
    pub config: Account<'info, RewardConfig>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub source_token_account: Account<'info, TokenAccount>,
    #[doc = "CHECK: created by this instruction at its derived address"]
    #[account(mut, seeds = [SPEND_SEED, payer.key().as_ref(), reference.as_ref()], bump)]
    pub receipt: UncheckedAccount<'info>,
    #[account(mut)]
    pub treasury_token_account: Option<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn process_spend_for_game(ctx: Context<SpendForGame>, amount: u64, reference: [u8; 32]) -> Result<()> {
    let config = &ctx.accounts.config;
    let payer = ctx.accounts.payer.key();

    require!(!config.paused, RewardError::Paused);
    require!(amount > 0, RewardError::AmountOutOfRange);
    require!(ctx.accounts.source_token_account.mint == config.mint, RewardError::WrongMint);
    require!(ctx.accounts.source_token_account.owner == payer, RewardError::InvalidAuthority);
    require!(
        is_unused_account(&ctx.accounts.receipt.to_account_info()),
        RewardError::ReferenceAlreadyUsed
    );

    let (burned, to_treasury) = split_spend(amount, config.spend_burn_bps)?;
    if to_treasury > 0 {
        let treasury = ctx
            .accounts
            .treasury_token_account
            .as_ref()
            .ok_or(RewardError::TreasuryMismatch)?;
        require!(config.treasury == Some(treasury.key()), RewardError::TreasuryMismatch);
        require!(treasury.mint == config.mint, RewardError::WrongMint);
    }

    let receipt = SpendReceipt {
        bump: ctx.bumps.receipt,
        payer,
        amount,
        burned,
        to_treasury,
        slot: Clock::get()?.slot,
    };
    create_state_account(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.receipt.to_account_info(),
        ctx.program_id,
        SpendReceipt::SPACE,
        &[SPEND_SEED, payer.as_ref(), reference.as_ref(), &[ctx.bumps.receipt]],
        &receipt,
    )?;

    let token_program_id = ctx.accounts.token_program.key();
    if burned > 0 {
        token::burn(
            CpiContext::new(
                token_program_id,
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.source_token_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            burned,
        )?;
    }
    if to_treasury > 0 {
        let treasury = ctx
            .accounts
            .treasury_token_account
            .as_ref()
            .ok_or(RewardError::TreasuryMismatch)?;
        token::transfer(
            CpiContext::new(
                token_program_id,
                Transfer {
                    from: ctx.accounts.source_token_account.to_account_info(),
                    to: treasury.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            to_treasury,
        )?;
    }

    ctx.accounts.mint.reload()?;
    emit_cpi!(GameSpend {
        payer,
        reference,
        amount,
        burned,
        to_treasury,
        supply_after: ctx.accounts.mint.supply,
    });
    Ok(())
}
