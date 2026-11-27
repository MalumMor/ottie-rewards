// SPDX-License-Identifier: LicenseRef-Proprietary
use anchor_lang::prelude::*;

#[event]
pub struct Initialized {
    pub admin: Pubkey,
    pub distributor: Pubkey,
    pub mint: Pubkey,
    pub max_supply: u64,
    pub decimals: u8,
}

#[event]
pub struct GenesisMinted {
    pub token_account: Pubkey,
    pub token_account_owner: Pubkey,
    pub amount: u64,
    pub supply_after: u64,
}

#[event]
pub struct DistributionOpened {
    pub period: u32,
    pub recipient_count: u16,
    pub allocated_total: u64,
    pub recipients_hash: [u8; 32],
    pub mint_not_before: i64,
    pub supply_before: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct MintedEntry {
    pub token_account: Pubkey,
    pub token_account_owner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct RewardsMinted {
    pub period: u32,
    pub first_index: u16,
    pub entries: Vec<MintedEntry>,
}

#[event]
pub struct DistributionClosed {
    pub period: u32,
    pub minted_total: u64,
    pub minted_count: u16,
    pub unminted: u16,
    pub by: Pubkey,
}

#[event]
pub struct PauseChanged {
    pub paused: bool,
    pub by: Pubkey,
}

#[event]
pub struct DistributorChanged {
    pub old: Pubkey,
    pub new: Pubkey,
}

#[event]
pub struct LimitsChanged {
    pub max_amount_per_distribution: u64,
    pub max_recipients_per_distribution: u16,
    pub max_amount_per_recipient: u64,
    pub min_seconds_between_openings: i64,
    pub mint_delay_seconds: i64,
}

#[event]
pub struct SpendPolicyChanged {
    pub spend_burn_bps: u16,
    pub treasury: Option<Pubkey>,
}

#[event]
pub struct AdminChanged {
    pub old: Pubkey,
    pub new: Pubkey,
}

#[event]
pub struct GameSpend {
    pub payer: Pubkey,
    pub reference: [u8; 32],
    pub amount: u64,
    pub burned: u64,
    pub to_treasury: u64,
    pub supply_after: u64,
}
