#![allow(ambiguous_glob_reexports)]

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod pyth;
pub mod state;

use instructions::*;
use state::{Condition, Side, Visibility};

declare_id!("HiErQ1fFikbgqEMjDD58trMaZ8XHGtSmztEJu31UZA9");

#[program]
pub mod anturix {
    use super::*;

    pub fn init_user_profile(ctx: Context<InitProfile>) -> Result<()> {
        instructions::init_profile::handler(ctx)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_duel(
        ctx: Context<CreateDuel>,
        visibility: Visibility,
        creator_side: Side,
        stake_amount: u64,
        condition: Condition,
        price_feed_id: [u8; 32],
        price_feed_id_b: [u8; 32],
        target_price: i64,
        lower_bound: i64,
        upper_bound: i64,
        expires_at: i64,
    ) -> Result<()> {
        instructions::create_duel::handler(
            ctx,
            visibility,
            creator_side,
            stake_amount,
            condition,
            price_feed_id,
            price_feed_id_b,
            target_price,
            lower_bound,
            upper_bound,
            expires_at,
        )
    }

    pub fn join_pool(ctx: Context<JoinPool>, side: Side, amount: u64) -> Result<()> {
        instructions::join_pool::handler(ctx, side, amount)
    }

    pub fn resolve_duel(ctx: Context<ResolveDuel>) -> Result<()> {
        instructions::resolve_duel::handler(ctx)
    }

    pub fn claim_share(ctx: Context<ClaimShare>, side: Side) -> Result<()> {
        instructions::claim_share::handler(ctx, side)
    }

    pub fn claim_refund(ctx: Context<ClaimRefund>, side: Side) -> Result<()> {
        instructions::claim_refund::handler(ctx, side)
    }

    pub fn cancel_duel(ctx: Context<CancelDuel>) -> Result<()> {
        instructions::cancel_duel::handler(ctx)
    }

    pub fn force_cancel_duel(ctx: Context<ForceCancelDuel>) -> Result<()> {
        instructions::force_cancel_duel::handler(ctx)
    }
}
