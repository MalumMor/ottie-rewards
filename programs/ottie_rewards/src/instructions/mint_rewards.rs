// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::token::spl_token::state::{Account as SplTokenAccount, AccountState};
use anchor_spl::token::{self, Mint, MintTo, Token};

use crate::errors::RewardError;
use crate::events::{MintedEntry, RewardsMinted};
use crate::state::*;

pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

const ASSOCIATED_TOKEN_CREATE: u8 = 0;

#[event_cpi]
#[derive(Accounts)]
#[instruction(period: u32)]
pub struct MintRewards<'info> {
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
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[doc = "CHECK: derived signer address, holds no data"]
    #[account(seeds = [MINT_AUTHORITY_SEED], bump = config.mint_authority_bump)]
    pub mint_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [DISTRIBUTION_SEED, &period.to_le_bytes()],
        bump = distribution.bump
    )]
    pub distribution: Account<'info, Distribution>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    #[doc = "CHECK: the fixed address of the Associated Token Account program (checked here); only ever invoked"]
    #[account(address = ASSOCIATED_TOKEN_PROGRAM_ID @ RewardError::InvalidTokenAccount)]
    pub associated_token_program: UncheckedAccount<'info>,
}

pub fn process_mint_rewards<'info>(
    ctx: Context<'info, MintRewards<'info>>,
    period: u32,
    amounts: Vec<u64>,
    tail_link: [u8; 32],
) -> Result<()> {
    let config = &ctx.accounts.config;
    let distribution = &ctx.accounts.distribution;
    let now = Clock::get()?.unix_timestamp;

    require!(!config.paused, RewardError::Paused);
    match config.open_period {
        None => return err!(RewardError::NoOpenDistribution),
        Some(open) => require!(open == period && distribution.period == period, RewardError::WrongPeriod),
    }
    require!(now >= distribution.mint_not_before, RewardError::MintDelayNotElapsed);

    let count = amounts.len();
    require!((1..=MAX_ENTRIES_PER_IX).contains(&count), RewardError::TooManyEntries);
    let expected_accounts = count.checked_mul(2).ok_or(RewardError::MathOverflow)?;
    require!(ctx.remaining_accounts.len() == expected_accounts, RewardError::EntryAccountMismatch);
    let first_index = distribution.minted_count;
    let end_index = (first_index as usize)
        .checked_add(count)
        .ok_or(RewardError::MathOverflow)?;
    require!(end_index <= distribution.recipient_count as usize, RewardError::IndexOutOfRange);

    let token_accounts: Vec<Pubkey> = ctx.remaining_accounts.chunks_exact(2).map(|pair| pair[0].key()).collect();
    let folded = fold_links(period, first_index, &token_accounts, &amounts, &tail_link)?;
    require!(folded == distribution.next_link, RewardError::CommitmentMismatch);
    if end_index == distribution.recipient_count as usize {
        require!(tail_link == EMPTY_LINK, RewardError::CommitmentMismatch);
    }

    let max_supply = config.max_supply;
    let max_amount_per_recipient = config.max_amount_per_recipient;
    let config_mint = config.mint;
    let allocated_total = distribution.allocated_total;
    let bump = [config.mint_authority_bump];
    let seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, &bump];
    let token_program_id = ctx.accounts.token_program.key();

    let mut minted_total = distribution.minted_total;
    let mut reward_total = config.total_reward_minted;
    let mut entries: Vec<MintedEntry> = Vec::with_capacity(count);

    for (offset, pair) in ctx.remaining_accounts.chunks_exact(2).enumerate() {
        let recipient = &pair[0];
        let wallet = &pair[1];
        let amount = amounts[offset];
        require!(
            (1..=max_amount_per_recipient).contains(&amount),
            RewardError::AmountOutOfRange
        );
        minted_total = minted_total.checked_add(amount).ok_or(RewardError::MathOverflow)?;
        require!(minted_total <= allocated_total, RewardError::DistributionCapExceeded);

        require!(recipient.is_writable, RewardError::InvalidTokenAccount);
        let exists = recipient.owner == &token_program_id && !recipient.data_is_empty();
        if !exists {
            invoke(
                &Instruction {
                    program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
                    accounts: vec![
                        AccountMeta::new(ctx.accounts.distributor.key(), true),
                        AccountMeta::new(recipient.key(), false),
                        AccountMeta::new_readonly(wallet.key(), false),
                        AccountMeta::new_readonly(config_mint, false),
                        AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
                        AccountMeta::new_readonly(token_program_id, false),
                    ],
                    data: vec![ASSOCIATED_TOKEN_CREATE],
                },
                &[
                    ctx.accounts.distributor.to_account_info(),
                    recipient.clone(),
                    wallet.clone(),
                    ctx.accounts.mint.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    ctx.accounts.associated_token_program.to_account_info(),
                ],
            )?;
        }
        require!(recipient.owner == &token_program_id, RewardError::InvalidTokenAccount);
        let token_account = {
            let data = recipient.try_borrow_data()?;
            SplTokenAccount::unpack(&data).map_err(|_| error!(RewardError::InvalidTokenAccount))?
        };
        require!(token_account.mint == config_mint, RewardError::WrongMint);
        require!(token_account.state != AccountState::Frozen, RewardError::FrozenAccount);

        ctx.accounts.mint.reload()?;
        let supply_after = ctx
            .accounts
            .mint
            .supply
            .checked_add(amount)
            .ok_or(RewardError::MathOverflow)?;
        require!(supply_after <= max_supply, RewardError::SupplyCapExceeded);

        token::mint_to(
            CpiContext::new_with_signer(
                token_program_id,
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: recipient.clone(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                &[seeds],
            ),
            amount,
        )?;

        reward_total = reward_total.checked_add(amount).ok_or(RewardError::MathOverflow)?;
        entries.push(MintedEntry {
            token_account: recipient.key(),
            token_account_owner: token_account.owner,
            amount,
        });
    }

    let distribution = &mut ctx.accounts.distribution;
    distribution.minted_total = minted_total;
    distribution.minted_count = u16::try_from(end_index).map_err(|_| RewardError::MathOverflow)?;
    distribution.next_link = tail_link;
    ctx.accounts.config.total_reward_minted = reward_total;

    emit_cpi!(RewardsMinted { period, first_index, entries });
    Ok(())
}
