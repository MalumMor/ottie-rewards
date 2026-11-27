// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

use crate::errors::RewardError;
use crate::events::SpendPolicyChanged;
use crate::state::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetSpendPolicy<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RewardError::NotAdmin
    )]
    pub config: Account<'info, RewardConfig>,
    pub treasury_token_account: Option<Account<'info, TokenAccount>>,
}

pub fn process_set_spend_policy(
    ctx: Context<SetSpendPolicy>,
    spend_burn_bps: u16,
    treasury: Option<Pubkey>,
) -> Result<()> {
    require!(spend_burn_bps <= BPS_DENOMINATOR, RewardError::InvalidSpendPolicy);
    let config = &mut ctx.accounts.config;
    if spend_burn_bps == BPS_DENOMINATOR {
        require!(
            treasury.is_none() && ctx.accounts.treasury_token_account.is_none(),
            RewardError::InvalidSpendPolicy
        );
    } else {
        let expected = treasury.ok_or(RewardError::InvalidSpendPolicy)?;
        let account = ctx
            .accounts
            .treasury_token_account
            .as_ref()
            .ok_or(RewardError::InvalidSpendPolicy)?;
        require!(account.key() == expected, RewardError::TreasuryMismatch);
        require!(account.mint == config.mint, RewardError::WrongMint);
    }
    config.spend_burn_bps = spend_burn_bps;
    config.treasury = treasury;

    emit_cpi!(SpendPolicyChanged { spend_burn_bps, treasury });
    Ok(())
}
