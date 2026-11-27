// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use solana_sha256_hasher::hashv;

use crate::errors::RewardError;

pub const CONFIG_SEED: &[u8] = b"config";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint_authority";
pub const DISTRIBUTION_SEED: &[u8] = b"distribution";
pub const SPEND_SEED: &[u8] = b"spend";
pub const ENTRY_DOMAIN: &[u8] = b"reward-entry-v1";

pub const LAYOUT_VERSION: u8 = 1;
pub const MAX_RECIPIENTS_HARD: u16 = 1024;
pub const MAX_ENTRIES_PER_IX: usize = 24;
pub const MIN_SECONDS_BETWEEN_OPENINGS_FLOOR: i64 = 1_728_000;
pub const MINT_DELAY_SECONDS_MIN: i64 = 900;
pub const MINT_DELAY_SECONDS_MAX: i64 = 86_400;
pub const MAX_DECIMALS: u8 = 9;
pub const BPS_DENOMINATOR: u16 = 10_000;
pub const SECONDS_PER_DAY: i64 = 86_400;
pub const EMPTY_LINK: [u8; 32] = [0u8; 32];

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_amount_per_distribution: u64,
    pub max_recipients_per_distribution: u16,
    pub max_amount_per_recipient: u64,
    pub min_seconds_between_openings: i64,
    pub mint_delay_seconds: i64,
}

#[account]
#[derive(InitSpace)]
pub struct RewardConfig {
    pub version: u8,
    pub bump: u8,
    pub mint_authority_bump: u8,
    pub admin: Pubkey,
    pub pending_admin: Option<Pubkey>,
    pub distributor: Pubkey,
    pub mint: Pubkey,
    pub max_supply: u64,
    pub genesis_minted: bool,
    pub genesis_closed: bool,
    pub paused: bool,
    pub max_amount_per_distribution: u64,
    pub max_recipients_per_distribution: u16,
    pub max_amount_per_recipient: u64,
    pub min_seconds_between_openings: i64,
    pub mint_delay_seconds: i64,
    pub last_period: u32,
    pub last_opened_at: i64,
    pub open_period: Option<u32>,
    pub total_genesis: u64,
    pub total_reward_minted: u64,
    pub spend_burn_bps: u16,
    pub treasury: Option<Pubkey>,
    pub reserved: [u8; 128],
}

impl RewardConfig {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct Distribution {
    pub bump: u8,
    pub period: u32,
    pub recipient_count: u16,
    pub allocated_total: u64,
    pub minted_total: u64,
    pub minted_count: u16,
    pub recipients_hash: [u8; 32],
    pub next_link: [u8; 32],
    pub opened_at: i64,
    pub mint_not_before: i64,
    pub closed_at: i64,
}

impl Distribution {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

#[account]
#[derive(InitSpace)]
pub struct SpendReceipt {
    pub bump: u8,
    pub payer: Pubkey,
    pub amount: u64,
    pub burned: u64,
    pub to_treasury: u64,
    pub slot: u64,
}

impl SpendReceipt {
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

pub fn validate_limits(limits: &Limits, max_supply: u64) -> Result<()> {
    require!(
        (1..=MAX_RECIPIENTS_HARD).contains(&limits.max_recipients_per_distribution),
        RewardError::InvalidLimits
    );
    require!(limits.max_amount_per_recipient > 0, RewardError::InvalidLimits);
    require!(
        (1..=max_supply).contains(&limits.max_amount_per_distribution),
        RewardError::InvalidLimits
    );
    require!(
        limits.min_seconds_between_openings >= MIN_SECONDS_BETWEEN_OPENINGS_FLOOR,
        RewardError::InvalidLimits
    );
    require!(
        (MINT_DELAY_SECONDS_MIN..=MINT_DELAY_SECONDS_MAX).contains(&limits.mint_delay_seconds),
        RewardError::InvalidLimits
    );
    Ok(())
}

pub fn civil_year_month(unix_seconds: i64) -> Result<(i64, u32)> {
    let days = unix_seconds.div_euclid(SECONDS_PER_DAY);
    let z = days.checked_add(719_468).ok_or(RewardError::MathOverflow)?;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    Ok((year, month as u32))
}

pub fn utc_period(unix_seconds: i64) -> Result<u32> {
    let (year, month) = civil_year_month(unix_seconds)?;
    require!((1..=9_999).contains(&year), RewardError::PeriodNotJustClosed);
    Ok(year as u32 * 100 + month)
}

pub fn previous_period(period: u32) -> Result<u32> {
    let year = period / 100;
    let month = period % 100;
    require!((1..=12).contains(&month) && year >= 1, RewardError::PeriodNotJustClosed);
    if month == 1 {
        require!(year >= 2, RewardError::PeriodNotJustClosed);
        Ok((year - 1) * 100 + 12)
    } else {
        Ok(year * 100 + month - 1)
    }
}

pub fn entry_link(
    period: u32,
    index: u16,
    token_account: &Pubkey,
    amount: u64,
    next: &[u8; 32],
) -> [u8; 32] {
    hashv(&[
        ENTRY_DOMAIN,
        &period.to_le_bytes(),
        &index.to_le_bytes(),
        token_account.as_ref(),
        &amount.to_le_bytes(),
        next,
    ])
    .to_bytes()
}

pub fn fold_links(
    period: u32,
    first_index: u16,
    token_accounts: &[Pubkey],
    amounts: &[u64],
    tail_link: &[u8; 32],
) -> Result<[u8; 32]> {
    require!(token_accounts.len() == amounts.len(), RewardError::EntryAccountMismatch);
    let mut link = *tail_link;
    for offset in (0..amounts.len()).rev() {
        let index = (first_index as usize)
            .checked_add(offset)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or(RewardError::MathOverflow)?;
        link = entry_link(period, index, &token_accounts[offset], amounts[offset], &link);
    }
    Ok(link)
}

pub fn split_spend(amount: u64, burn_bps: u16) -> Result<(u64, u64)> {
    require!(burn_bps <= BPS_DENOMINATOR, RewardError::InvalidSpendPolicy);
    let burned = (amount as u128)
        .checked_mul(burn_bps as u128)
        .ok_or(RewardError::MathOverflow)?
        / BPS_DENOMINATOR as u128;
    let burned = u64::try_from(burned).map_err(|_| RewardError::MathOverflow)?;
    let to_treasury = amount.checked_sub(burned).ok_or(RewardError::MathOverflow)?;
    Ok((burned, to_treasury))
}

pub fn is_unused_account(account: &AccountInfo) -> bool {
    account.owner == &system_program::ID && account.data_is_empty()
}

pub fn create_state_account<'info, T: AccountSerialize>(
    payer: &AccountInfo<'info>,
    target: &AccountInfo<'info>,
    program_id: &Pubkey,
    space: usize,
    seeds: &[&[u8]],
    state: &T,
) -> Result<()> {
    let required = Rent::get()?.minimum_balance(space);
    let signer: &[&[&[u8]]] = &[seeds];
    if target.lamports() == 0 {
        system_program::create_account(
            CpiContext::new_with_signer(
                system_program::ID,
                system_program::CreateAccount { from: payer.clone(), to: target.clone() },
                signer,
            ),
            required,
            space as u64,
            program_id,
        )?;
    } else {
        let shortfall = required.saturating_sub(target.lamports());
        if shortfall > 0 {
            system_program::transfer(
                CpiContext::new(
                    system_program::ID,
                    system_program::Transfer { from: payer.clone(), to: target.clone() },
                ),
                shortfall,
            )?;
        }
        system_program::allocate(
            CpiContext::new_with_signer(
                system_program::ID,
                system_program::Allocate { account_to_allocate: target.clone() },
                signer,
            ),
            space as u64,
        )?;
        system_program::assign(
            CpiContext::new_with_signer(
                system_program::ID,
                system_program::Assign { account_to_assign: target.clone() },
                signer,
            ),
            program_id,
        )?;
    }
    let mut data = target.try_borrow_mut_data()?;
    let mut cursor: &mut [u8] = &mut data;
    state.try_serialize(&mut cursor)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix(year: i64, month: i64, day: i64, seconds_into_day: i64) -> i64 {
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let mp = if month > 2 { month - 3 } else { month + 9 };
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        (era * 146_097 + doe - 719_468) * SECONDS_PER_DAY + seconds_into_day
    }

    #[test]
    fn period_at_month_edges() {
        assert_eq!(utc_period(0).unwrap(), 197_001);
        assert_eq!(utc_period(unix(2026, 12, 31, 86_399)).unwrap(), 202_612);
        assert_eq!(utc_period(unix(2027, 1, 1, 0)).unwrap(), 202_701);
        assert_eq!(utc_period(unix(2027, 2, 28, 86_399)).unwrap(), 202_702);
        assert_eq!(utc_period(unix(2027, 3, 1, 0)).unwrap(), 202_703);
        assert_eq!(utc_period(unix(2028, 2, 29, 0)).unwrap(), 202_802);
        assert_eq!(utc_period(unix(2028, 2, 29, 86_399)).unwrap(), 202_802);
        assert_eq!(utc_period(unix(2028, 3, 1, 0)).unwrap(), 202_803);
        assert_eq!(utc_period(unix(2100, 2, 28, 86_399)).unwrap(), 210_002);
        assert_eq!(utc_period(unix(2100, 3, 1, 0)).unwrap(), 210_003);
        assert_eq!(utc_period(unix(2400, 2, 29, 12)).unwrap(), 240_002);
    }

    #[test]
    fn period_matches_every_month_start_and_end() {
        for year in 1970..=2200 {
            for month in 1..=12 {
                let start = unix(year, month, 1, 0);
                assert_eq!(utc_period(start).unwrap(), (year * 100 + month) as u32);
                let previous = previous_period((year * 100 + month) as u32).unwrap();
                if start > 0 {
                    assert_eq!(utc_period(start - 1).unwrap(), previous);
                }
            }
        }
    }

    #[test]
    fn previous_period_rolls_over_years() {
        assert_eq!(previous_period(202_701).unwrap(), 202_612);
        assert_eq!(previous_period(202_612).unwrap(), 202_611);
        assert_eq!(previous_period(202_703).unwrap(), 202_702);
        assert!(previous_period(202_700).is_err());
        assert!(previous_period(202_713).is_err());
    }

    #[test]
    fn spend_split_rounds_down() {
        assert_eq!(split_spend(1, 10_000).unwrap(), (1, 0));
        assert_eq!(split_spend(1, 9_999).unwrap(), (0, 1));
        assert_eq!(split_spend(3, 1).unwrap(), (0, 3));
        assert_eq!(split_spend(10_001, 1).unwrap(), (1, 10_000));
        assert_eq!(split_spend(999_999, 7_000).unwrap(), (699_999, 300_000));
        assert_eq!(split_spend(u64::MAX, 10_000).unwrap(), (u64::MAX, 0));
        assert_eq!(split_spend(u64::MAX, 0).unwrap(), (0, u64::MAX));
        assert!(split_spend(1, 10_001).is_err());
    }

    #[test]
    fn fold_reaches_head_from_any_batch_split() {
        let period = 202_610u32;
        let accounts: Vec<Pubkey> = (0..50u8).map(|seed| Pubkey::new_from_array([seed + 1; 32])).collect();
        let amounts: Vec<u64> = (0..50u64).map(|value| (value + 1) * 1_000_000).collect();
        let mut links = vec![EMPTY_LINK; 51];
        for index in (0..50usize).rev() {
            links[index] = entry_link(period, index as u16, &accounts[index], amounts[index], &links[index + 1]);
        }
        for batch in [1usize, 7, 24, 50] {
            let mut cursor = 0usize;
            while cursor < 50 {
                let end = usize::min(cursor + batch, 50);
                let folded = fold_links(period, cursor as u16, &accounts[cursor..end], &amounts[cursor..end], &links[end]).unwrap();
                assert_eq!(folded, links[cursor]);
                cursor = end;
            }
        }
        let mut swapped = accounts.clone();
        swapped.swap(0, 1);
        assert_ne!(fold_links(period, 0, &swapped[0..2], &amounts[0..2], &links[2]).unwrap(), links[0]);
        let mut changed = amounts.clone();
        changed[0] += 1;
        assert_ne!(fold_links(period, 0, &accounts[0..2], &changed[0..2], &links[2]).unwrap(), links[0]);
        assert_ne!(fold_links(period, 1, &accounts[0..2], &amounts[0..2], &links[2]).unwrap(), links[0]);
        assert_ne!(fold_links(period + 1, 0, &accounts[0..2], &amounts[0..2], &links[2]).unwrap(), links[0]);
        assert_ne!(fold_links(period, 0, &accounts[0..2], &amounts[0..2], &EMPTY_LINK).unwrap(), links[0]);
    }
}
