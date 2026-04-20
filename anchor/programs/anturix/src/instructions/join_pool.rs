use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::{UserProfile, DuelState, DuelStatus, Visibility, Side, Position};
use crate::constants::*;
use crate::errors::AnturixError;
use crate::events::PoolJoined;

pub fn handler(ctx: Context<JoinPool>, side: Side, amount: u64) -> Result<()> {
    let duel = &mut ctx.accounts.duel_state;

    require!(duel.status == DuelStatus::Open, AnturixError::InvalidDuelStatus);

    let clock = Clock::get()?;
    require!(clock.unix_timestamp < duel.expires_at, AnturixError::DuelExpired);

    let signer_key = ctx.accounts.participant.key();
    require!(signer_key != duel.creator, AnturixError::SelfDuel);

    let position = &mut ctx.accounts.participant_position;
    let is_first_entry = position.stake == 0;

    match duel.visibility {
        Visibility::Private => {
            require!(side == duel.creator_side.opposite(), AnturixError::WrongSide);
            require!(amount == duel.creator_stake, AnturixError::ExactStakeRequired);
            let opposite_total = match side {
                Side::OptionA => duel.side_a_total,
                Side::OptionB => duel.side_b_total,
            };
            require!(opposite_total == 0, AnturixError::PrivateAlreadyMatched);
        }
        Visibility::Public => {
            require!(amount >= MIN_JOIN_STAKE, AnturixError::StakeTooLow);
        }
    }

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            system_program::Transfer {
                from: ctx.accounts.participant.to_account_info(),
                to: ctx.accounts.escrow.to_account_info(),
            },
        ),
        amount,
    )?;

    if is_first_entry {
        position.duel = duel.key();
        position.owner = signer_key;
        position.side = side;
        position.bump = ctx.bumps.participant_position;
    }
    position.stake = position.stake.checked_add(amount).ok_or(AnturixError::Overflow)?;

    match side {
        Side::OptionA => {
            duel.side_a_total = duel.side_a_total.checked_add(amount).ok_or(AnturixError::Overflow)?;
        }
        Side::OptionB => {
            duel.side_b_total = duel.side_b_total.checked_add(amount).ok_or(AnturixError::Overflow)?;
        }
    }

    emit!(PoolJoined {
        duel: duel.key(),
        owner: signer_key,
        side: side as u8,
        amount,
        new_position_stake: position.stake,
        side_a_total: duel.side_a_total,
        side_b_total: duel.side_b_total,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(side: Side, amount: u64)]
pub struct JoinPool<'info> {
    #[account(mut)]
    pub participant: Signer<'info>,

    #[account(
        seeds = [SEED_PROFILE, participant.key().as_ref()],
        bump = participant_profile.bump,
    )]
    pub participant_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub duel_state: Account<'info, DuelState>,

    #[account(
        init_if_needed,
        payer = participant,
        space = Position::SIZE,
        seeds = [
            SEED_POSITION,
            duel_state.key().as_ref(),
            participant.key().as_ref(),
            &[side as u8],
        ],
        bump,
    )]
    pub participant_position: Account<'info, Position>,

    /// CHECK: escrow PDA
    #[account(
        mut,
        seeds = [SEED_ESCROW, duel_state.key().as_ref()],
        bump = duel_state.escrow_bump,
    )]
    pub escrow: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}
