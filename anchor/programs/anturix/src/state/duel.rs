use anchor_lang::prelude::*;

#[account]
pub struct DuelState {
    pub creator: Pubkey,              // 32
    pub opponent: Pubkey,             // 32 — default = public pool, set = targeted/accepted
    pub price_feed_id: [u8; 32],      // 32 — Pyth price feed ID (asset A)
    pub target_price: i64,            // 8  — target price (Above/Below)
    pub condition: Condition,         // 1  — bet type
    pub stake_amount: u64,            // 8
    pub status: DuelStatus,           // 1
    pub winner: Option<Pubkey>,       // 1 + 32 = 33
    pub expires_at: i64,              // 8  — settlement allowed after this timestamp
    pub bump: u8,                     // 1
    pub escrow_bump: u8,              // 1
    pub lower_bound: i64,             // 8  — InRange/OutOfRange lower bound
    pub upper_bound: i64,             // 8  — InRange/OutOfRange upper bound
    pub price_feed_id_b: [u8; 32],    // 32 — AssetRace second feed
    pub start_price_a: i64,           // 8  — AssetRace snapshot at creation
    pub start_price_b: i64,           // 8  — AssetRace snapshot at creation
}

impl DuelState {
    pub const SIZE: usize = 8 + 32 + 32 + 32 + 8 + 1 + 8 + 1 + (1 + 32) + 8 + 1 + 1 + 8 + 8 + 32 + 8 + 8; // 229
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum DuelStatus {
    Pending,
    Active,
    Resolved,
    Cancelled,
    Claimed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum Condition {
    Above,      // creator wins if price > target_price
    Below,      // creator wins if price < target_price
    Odd,        // creator wins if last digit of price is odd
    Even,       // creator wins if last digit of price is even
    InRange,    // creator wins if lower_bound <= price <= upper_bound
    OutOfRange, // creator wins if price < lower_bound OR price > upper_bound
    AssetRace,  // creator wins if asset A outperforms asset B (% gain)
}
