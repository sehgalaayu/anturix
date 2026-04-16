use anchor_lang::prelude::*;
use crate::state::{DuelState, DuelStatus, Condition, UserProfile};
use crate::constants::*;
use crate::errors::AnturixError;
use crate::events::DuelResolved;
use crate::pyth;

/// Permissionless -- anyone (cranker, creator, opponent) can call this.
/// The Pyth oracle determines the winner automatically.
pub fn handler(ctx: Context<ResolveDuel>) -> Result<()> {
    let duel = &mut ctx.accounts.duel_state;

    require!(duel.status == DuelStatus::Active, AnturixError::InvalidDuelStatus);

    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp >= duel.expires_at,
        AnturixError::DuelNotExpired
    );

    // Read primary oracle price
    let pyth_price = pyth::parse_price_update(
        &ctx.accounts.price_update.to_account_info(),
        &duel.price_feed_id,
        &clock,
    )?;

    // Determine winner based on condition
    let condition_met = match duel.condition {
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
            // Read second feed from remaining_accounts
            require!(
                !ctx.remaining_accounts.is_empty(),
                AnturixError::MissingPriceAccount
            );
            let pyth_price_b = pyth::parse_price_update(
                &ctx.remaining_accounts[0],
                &duel.price_feed_id_b,
                &clock,
            )?;

            // % gain in basis points: (end - start) * 10000 / start
            // Using i64 arithmetic. Negative gain = price dropped.
            let gain_a = (pyth_price.price.checked_sub(duel.start_price_a).ok_or(AnturixError::Overflow)?)
                .checked_mul(10000).ok_or(AnturixError::Overflow)?
                .checked_div(duel.start_price_a).ok_or(AnturixError::Overflow)?;

            let gain_b = (pyth_price_b.price.checked_sub(duel.start_price_b).ok_or(AnturixError::Overflow)?)
                .checked_mul(10000).ok_or(AnturixError::Overflow)?
                .checked_div(duel.start_price_b).ok_or(AnturixError::Overflow)?;

            // Creator's asset A outperformed asset B
            gain_a > gain_b
        }
    };

    let (winner, loser) = if condition_met {
        (duel.creator, duel.opponent)
    } else {
        (duel.opponent, duel.creator)
    };

    duel.winner = Some(winner);
    duel.status = DuelStatus::Resolved;

    // Update winner profile
    let winner_profile = if winner == duel.creator {
        &mut ctx.accounts.creator_profile
    } else {
        &mut ctx.accounts.opponent_profile
    };
    winner_profile.wins = winner_profile.wins
        .checked_add(1)
        .ok_or(AnturixError::Overflow)?;

    // Update loser profile
    let loser_profile = if loser == duel.creator {
        &mut ctx.accounts.creator_profile
    } else {
        &mut ctx.accounts.opponent_profile
    };
    loser_profile.losses = loser_profile.losses
        .checked_add(1)
        .ok_or(AnturixError::Overflow)?;
    loser_profile.clown_until = clock.unix_timestamp
        .checked_add(CLOWN_DURATION)
        .ok_or(AnturixError::Overflow)?;

    emit!(DuelResolved {
        duel: duel.key(),
        winner,
        loser,
        oracle_price: pyth_price.price,
        oracle_exponent: pyth_price.exponent,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ResolveDuel<'info> {
    /// Anyone can resolve -- permissionless (cranker, participant, etc.)
    pub resolver: Signer<'info>,

    #[account(mut)]
    pub duel_state: Account<'info, DuelState>,

    /// CHECK: Pyth PriceUpdateV2 account -- validated in handler via pyth::parse_price_update
    pub price_update: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SEED_PROFILE, duel_state.creator.as_ref()],
        bump = creator_profile.bump,
    )]
    pub creator_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [SEED_PROFILE, duel_state.opponent.as_ref()],
        bump = opponent_profile.bump,
    )]
    pub opponent_profile: Account<'info, UserProfile>,
}
