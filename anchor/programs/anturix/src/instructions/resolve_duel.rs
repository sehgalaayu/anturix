use anchor_lang::prelude::*;
use crate::state::{DuelState, DuelStatus, Condition, Side};
use crate::errors::AnturixError;
use crate::events::{DuelResolved, DuelCancelled};
use crate::pyth;

/// Permissionless. Anyone can crank after expires_at.
/// Orphan side auto-cancels; otherwise Pyth determines winner_side.
pub fn handler(ctx: Context<ResolveDuel>) -> Result<()> {
    let duel = &mut ctx.accounts.duel_state;

    require!(duel.status == DuelStatus::Open, AnturixError::InvalidDuelStatus);

    let clock = Clock::get()?;
    require!(clock.unix_timestamp >= duel.expires_at, AnturixError::DuelNotExpired);

    // Orphan check: if either side has no liquidity, cancel the whole duel.
    if duel.side_a_total == 0 || duel.side_b_total == 0 {
        duel.status = DuelStatus::Cancelled;
        emit!(DuelCancelled {
            duel: duel.key(),
            reason: "orphan".to_string(),
        });
        return Ok(());
    }

    let pyth_price = pyth::parse_price_update(
        &ctx.accounts.price_update.to_account_info(),
        &duel.price_feed_id,
        &clock,
    )?;
    require!(
        pyth_price.publish_time >= duel.expires_at,
        AnturixError::PriceBeforeExpiry
    );

    let option_a_wins = match duel.condition {
        Condition::Above => pyth_price.price > duel.target_price,
        Condition::Below => pyth_price.price < duel.target_price,
        Condition::Odd => {
            let last_digit = (pyth_price.price % 10).unsigned_abs();
            last_digit % 2 == 1
        }
        Condition::Even => {
            let last_digit = (pyth_price.price % 10).unsigned_abs();
            last_digit % 2 == 0
        }
        Condition::InRange => {
            pyth_price.price >= duel.lower_bound && pyth_price.price <= duel.upper_bound
        }
        Condition::OutOfRange => {
            pyth_price.price < duel.lower_bound || pyth_price.price > duel.upper_bound
        }
        Condition::AssetRace => {
            require!(
                !ctx.remaining_accounts.is_empty(),
                AnturixError::MissingPriceAccount
            );
            let pyth_price_b = pyth::parse_price_update(
                &ctx.remaining_accounts[0],
                &duel.price_feed_id_b,
                &clock,
            )?;
            require!(
                pyth_price_b.publish_time >= duel.expires_at,
                AnturixError::PriceBeforeExpiry
            );

            let gain_a = (pyth_price.price.checked_sub(duel.start_price_a).ok_or(AnturixError::Overflow)?)
                .checked_mul(10000).ok_or(AnturixError::Overflow)?
                .checked_div(duel.start_price_a).ok_or(AnturixError::Overflow)?;

            let gain_b = (pyth_price_b.price.checked_sub(duel.start_price_b).ok_or(AnturixError::Overflow)?)
                .checked_mul(10000).ok_or(AnturixError::Overflow)?
                .checked_div(duel.start_price_b).ok_or(AnturixError::Overflow)?;

            gain_a > gain_b
        }
    };

    let winner_side = if option_a_wins { Side::OptionA } else { Side::OptionB };

    duel.winner_side = Some(winner_side);
    duel.oracle_price = pyth_price.price;
    duel.status = DuelStatus::Resolved;

    emit!(DuelResolved {
        duel: duel.key(),
        winner_side: winner_side as u8,
        oracle_price: pyth_price.price,
        oracle_exponent: pyth_price.exponent,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ResolveDuel<'info> {
    pub resolver: Signer<'info>,

    #[account(mut)]
    pub duel_state: Account<'info, DuelState>,

    /// CHECK: Pyth PriceUpdateV2 account, validated in handler
    pub price_update: UncheckedAccount<'info>,
}
