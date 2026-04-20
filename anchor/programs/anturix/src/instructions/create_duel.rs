use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::{UserProfile, DuelState, DuelStatus, Condition, Visibility, Side, Position};
use crate::constants::*;
use crate::errors::AnturixError;
use crate::events::DuelCreated;
use crate::pyth;

#[allow(clippy::too_many_arguments)]
pub fn handler(
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
    require!(stake_amount >= MIN_CREATE_STAKE, AnturixError::StakeTooLow);

    let clock = Clock::get()?;
    require!(
        expires_at >= clock.unix_timestamp.checked_add(MIN_EXPIRY_DURATION).ok_or(AnturixError::Overflow)?,
        AnturixError::InvalidExpiry
    );

    require!(price_feed_id != [0u8; 32], AnturixError::InvalidFeedId);

    let mut start_price_a: i64 = 0;
    let mut start_price_b: i64 = 0;

    match condition {
        Condition::Above | Condition::Below => {
            require!(target_price > 0, AnturixError::InvalidTargetPrice);
        }
        Condition::Odd | Condition::Even => {}
        Condition::InRange | Condition::OutOfRange => {
            require!(lower_bound > 0, AnturixError::InvalidBounds);
            require!(upper_bound > lower_bound, AnturixError::InvalidBounds);
        }
        Condition::AssetRace => {
            require!(price_feed_id_b != [0u8; 32], AnturixError::InvalidSecondFeed);
            require!(price_feed_id != price_feed_id_b, AnturixError::InvalidSecondFeed);
            require!(ctx.remaining_accounts.len() >= 2, AnturixError::MissingPriceAccount);

            let price_a = pyth::parse_price_update(
                &ctx.remaining_accounts[0],
                &price_feed_id,
                &clock,
            )?;
            let price_b = pyth::parse_price_update(
                &ctx.remaining_accounts[1],
                &price_feed_id_b,
                &clock,
            )?;

            start_price_a = price_a.price;
            start_price_b = price_b.price;
        }
    }

    let creator_profile = &mut ctx.accounts.creator_profile;
    let duel = &mut ctx.accounts.duel_state;

    duel.creator = ctx.accounts.creator.key();
    duel.visibility = visibility;
    duel.condition = condition;
    duel.price_feed_id = price_feed_id;
    duel.price_feed_id_b = price_feed_id_b;
    duel.target_price = target_price;
    duel.lower_bound = lower_bound;
    duel.upper_bound = upper_bound;
    duel.start_price_a = start_price_a;
    duel.start_price_b = start_price_b;
    duel.creator_side = creator_side;
    duel.creator_stake = stake_amount;
    duel.side_a_total = match creator_side {
        Side::OptionA => stake_amount,
        Side::OptionB => 0,
    };
    duel.side_b_total = match creator_side {
        Side::OptionA => 0,
        Side::OptionB => stake_amount,
    };
    duel.status = DuelStatus::Open;
    duel.winner_side = None;
    duel.oracle_price = 0;
    duel.expires_at = expires_at;
    duel.bump = ctx.bumps.duel_state;
    duel.escrow_bump = ctx.bumps.escrow;

    let position = &mut ctx.accounts.creator_position;
    position.duel = duel.key();
    position.owner = ctx.accounts.creator.key();
    position.side = creator_side;
    position.stake = stake_amount;
    position.bump = ctx.bumps.creator_position;

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            system_program::Transfer {
                from: ctx.accounts.creator.to_account_info(),
                to: ctx.accounts.escrow.to_account_info(),
            },
        ),
        stake_amount,
    )?;

    creator_profile.duel_count = creator_profile.duel_count
        .checked_add(1)
        .ok_or(AnturixError::Overflow)?;

    emit!(DuelCreated {
        duel: duel.key(),
        creator: ctx.accounts.creator.key(),
        visibility: visibility as u8,
        creator_side: creator_side as u8,
        condition: condition as u8,
        price_feed_id,
        price_feed_id_b,
        target_price,
        lower_bound,
        upper_bound,
        stake_amount,
        expires_at,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(
    visibility: Visibility,
    creator_side: Side,
)]
pub struct CreateDuel<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [SEED_PROFILE, creator.key().as_ref()],
        bump = creator_profile.bump,
    )]
    pub creator_profile: Account<'info, UserProfile>,

    #[account(
        init,
        payer = creator,
        space = DuelState::SIZE,
        seeds = [SEED_DUEL, creator.key().as_ref(), &creator_profile.duel_count.to_le_bytes()],
        bump,
    )]
    pub duel_state: Account<'info, DuelState>,

    #[account(
        init,
        payer = creator,
        space = Position::SIZE,
        seeds = [
            SEED_POSITION,
            duel_state.key().as_ref(),
            creator.key().as_ref(),
            &[creator_side as u8],
        ],
        bump,
    )]
    pub creator_position: Account<'info, Position>,

    /// CHECK: escrow PDA — system-owned, holds stake SOL
    #[account(
        mut,
        seeds = [SEED_ESCROW, duel_state.key().as_ref()],
        bump,
    )]
    pub escrow: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}
