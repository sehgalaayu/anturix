#![allow(dead_code)]

use anchor_lang::{InstructionData, ToAccountMetas, AccountDeserialize};
pub use litesvm::LiteSVM;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::{Instruction, AccountMeta},
    account::Account,
    clock::Clock,
};

pub use anchor_lang::prelude::Pubkey as AnchorPubkey;
pub use anturix::state::{Condition, Side, Visibility};

// ── Pyth constants ──

pub const PYTH_RECEIVER_ID: Pubkey = solana_sdk::pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

pub const SOL_USD_FEED: [u8; 32] = [
    0xef, 0x0d, 0x8b, 0x6f, 0xda, 0x2c, 0xeb, 0xa4,
    0x1d, 0xa1, 0x5d, 0x40, 0x95, 0xd1, 0xda, 0x39,
    0x2a, 0x0d, 0x2f, 0x8e, 0xd0, 0xc6, 0xc7, 0xbc,
    0x0f, 0x4c, 0xfa, 0xc8, 0xc2, 0x80, 0xb5, 0x6d,
];

pub const BTC_USD_FEED: [u8; 32] = [
    0xe6, 0x2d, 0xf6, 0xc8, 0xb4, 0xa8, 0x5f, 0xe1,
    0xa6, 0x7d, 0xb4, 0x4d, 0xc1, 0x2d, 0xe5, 0xdb,
    0x33, 0x0f, 0x7a, 0xc6, 0x6b, 0x72, 0xdc, 0x65,
    0x8a, 0xfe, 0xdf, 0x0f, 0x4a, 0x41, 0x5b, 0x43,
];

// ── Pubkey conversion ──

fn ap(pk: &Pubkey) -> AnchorPubkey {
    AnchorPubkey::new_from_array(pk.to_bytes())
}

fn sys_id() -> AnchorPubkey {
    AnchorPubkey::new_from_array([0u8; 32])
}

pub fn prog_id() -> Pubkey {
    Pubkey::new_from_array(anturix::id().to_bytes())
}

pub fn convert_account_metas(metas: Vec<anchor_lang::prelude::AccountMeta>) -> Vec<AccountMeta> {
    metas.into_iter().map(|m| {
        let pubkey = Pubkey::new_from_array(m.pubkey.to_bytes());
        if m.is_writable {
            AccountMeta::new(pubkey, m.is_signer)
        } else {
            AccountMeta::new_readonly(pubkey, m.is_signer)
        }
    }).collect()
}

fn build_ix<T: ToAccountMetas>(accounts: T, data: Vec<u8>) -> Instruction {
    let anchor_metas = accounts.to_account_metas(None);
    Instruction {
        program_id: prog_id(),
        accounts: convert_account_metas(anchor_metas),
        data,
    }
}

// ── PDA helpers ──

pub fn profile_pda(owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", owner.as_ref()], &prog_id())
}

pub fn duel_pda(creator: &Pubkey, duel_count: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"duel", creator.as_ref(), &duel_count.to_le_bytes()],
        &prog_id(),
    )
}

pub fn escrow_pda(duel_state: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"escrow", duel_state.as_ref()], &prog_id())
}

pub fn position_pda(duel_state: &Pubkey, owner: &Pubkey, side: Side) -> (Pubkey, u8) {
    let side_byte: u8 = match side {
        Side::OptionA => 0,
        Side::OptionB => 1,
    };
    Pubkey::find_program_address(
        &[b"position", duel_state.as_ref(), owner.as_ref(), &[side_byte]],
        &prog_id(),
    )
}

// ── SVM setup ──

pub fn setup() -> LiteSVM {
    let mut svm = LiteSVM::new();
    let program_bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy/anturix.so"));
    let _ = svm.add_program(prog_id(), program_bytes);
    svm
}

pub fn airdrop(svm: &mut LiteSVM, to: &Pubkey, lamports: u64) {
    svm.airdrop(to, lamports).unwrap();
}

pub fn fund_user(svm: &mut LiteSVM, user: &Keypair) {
    airdrop(svm, &user.pubkey(), 100_000_000_000); // 100 SOL
}

pub fn send_tx(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(ixs, Some(&payer.pubkey()), signers, blockhash);
    svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}", e))
}

pub fn send_tx_expect_err(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> String {
    send_tx(svm, ixs, payer, signers).expect_err("expected error but tx succeeded")
}

// ── Pyth mock ──

pub fn build_mock_price_update(
    feed_id: &[u8; 32],
    price: i64,
    conf: u64,
    exponent: i32,
    publish_time: i64,
) -> Vec<u8> {
    let discriminator: [u8; 8] = [0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd];
    let write_authority = [0u8; 32];
    let verification_level: u8 = 1;

    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&write_authority);
    data.extend_from_slice(&[verification_level]);
    data.extend_from_slice(feed_id);
    data.extend_from_slice(&price.to_le_bytes());
    data.extend_from_slice(&conf.to_le_bytes());
    data.extend_from_slice(&exponent.to_le_bytes());
    data.extend_from_slice(&publish_time.to_le_bytes());
    data.extend_from_slice(&(publish_time - 1).to_le_bytes());
    data.extend_from_slice(&price.to_le_bytes());
    data.extend_from_slice(&conf.to_le_bytes());
    data.extend_from_slice(&100u64.to_le_bytes());
    data
}

pub fn create_mock_pyth_account(
    svm: &mut LiteSVM,
    feed_id: &[u8; 32],
    price: i64,
    conf: u64,
    exponent: i32,
    publish_time: i64,
) -> Pubkey {
    let data = build_mock_price_update(feed_id, price, conf, exponent, publish_time);
    let key = Keypair::new().pubkey();
    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: PYTH_RECEIVER_ID,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(key, account).unwrap();
    key
}

pub fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
    let clock = Clock {
        slot: 100,
        epoch_start_timestamp: unix_timestamp,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp,
    };
    svm.set_sysvar(&clock);
}

pub fn admin_pubkey() -> Pubkey {
    solana_sdk::pubkey!("8RAViABqHQkdSesxZqqFcWnEYnw5baVN2AQB2Z2CmQgX")
}

// ── Instruction builders ──

pub fn ix_init_profile(owner: &Pubkey) -> Instruction {
    let (profile, _) = profile_pda(owner);
    build_ix(
        anturix::accounts::InitProfile {
            owner: ap(owner),
            user_profile: ap(&profile),
            system_program: sys_id(),
        },
        anturix::instruction::InitUserProfile {}.data(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn ix_create_duel(
    creator: &Pubkey,
    duel_count: u64,
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
    remaining: &[Pubkey],
) -> Instruction {
    let (profile, _) = profile_pda(creator);
    let (duel, _) = duel_pda(creator, duel_count);
    let (escrow, _) = escrow_pda(&duel);
    let (cpos, _) = position_pda(&duel, creator, creator_side);

    let anchor_accounts = anturix::accounts::CreateDuel {
        creator: ap(creator),
        creator_profile: ap(&profile),
        duel_state: ap(&duel),
        creator_position: ap(&cpos),
        escrow: ap(&escrow),
        system_program: sys_id(),
    };
    let mut metas = convert_account_metas(anchor_accounts.to_account_metas(None));
    for key in remaining {
        metas.push(AccountMeta::new_readonly(*key, false));
    }

    Instruction {
        program_id: prog_id(),
        accounts: metas,
        data: anturix::instruction::CreateDuel {
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
        }.data(),
    }
}

pub fn ix_join_pool(
    participant: &Pubkey,
    duel_state: &Pubkey,
    side: Side,
    amount: u64,
) -> Instruction {
    let (profile, _) = profile_pda(participant);
    let (escrow, _) = escrow_pda(duel_state);
    let (pos, _) = position_pda(duel_state, participant, side);

    build_ix(
        anturix::accounts::JoinPool {
            participant: ap(participant),
            participant_profile: ap(&profile),
            duel_state: ap(duel_state),
            participant_position: ap(&pos),
            escrow: ap(&escrow),
            system_program: sys_id(),
        },
        anturix::instruction::JoinPool { side, amount }.data(),
    )
}

pub fn ix_resolve_duel(
    resolver: &Pubkey,
    duel_state: &Pubkey,
    price_update: &Pubkey,
    remaining: &[Pubkey],
) -> Instruction {
    let anchor_accounts = anturix::accounts::ResolveDuel {
        resolver: ap(resolver),
        duel_state: ap(duel_state),
        price_update: ap(price_update),
    };
    let mut metas = convert_account_metas(anchor_accounts.to_account_metas(None));
    for key in remaining {
        metas.push(AccountMeta::new_readonly(*key, false));
    }
    Instruction {
        program_id: prog_id(),
        accounts: metas,
        data: anturix::instruction::ResolveDuel {}.data(),
    }
}

pub fn ix_claim_share(owner: &Pubkey, duel_state: &Pubkey, side: Side) -> Instruction {
    let (profile, _) = profile_pda(owner);
    let (escrow, _) = escrow_pda(duel_state);
    let (pos, _) = position_pda(duel_state, owner, side);

    build_ix(
        anturix::accounts::ClaimShare {
            owner: ap(owner),
            owner_profile: ap(&profile),
            duel_state: ap(duel_state),
            position: ap(&pos),
            escrow: ap(&escrow),
            system_program: sys_id(),
        },
        anturix::instruction::ClaimShare { side }.data(),
    )
}

pub fn ix_claim_refund(owner: &Pubkey, duel_state: &Pubkey, side: Side) -> Instruction {
    let (escrow, _) = escrow_pda(duel_state);
    let (pos, _) = position_pda(duel_state, owner, side);

    build_ix(
        anturix::accounts::ClaimRefund {
            owner: ap(owner),
            duel_state: ap(duel_state),
            position: ap(&pos),
            escrow: ap(&escrow),
            system_program: sys_id(),
        },
        anturix::instruction::ClaimRefund { side }.data(),
    )
}

pub fn ix_cancel_duel(creator: &Pubkey, duel_state: &Pubkey) -> Instruction {
    build_ix(
        anturix::accounts::CancelDuel {
            creator: ap(creator),
            duel_state: ap(duel_state),
        },
        anturix::instruction::CancelDuel {}.data(),
    )
}

pub fn ix_force_cancel_duel(admin: &Pubkey, duel_state: &Pubkey) -> Instruction {
    build_ix(
        anturix::accounts::ForceCancelDuel {
            admin: ap(admin),
            duel_state: ap(duel_state),
        },
        anturix::instruction::ForceCancelDuel {}.data(),
    )
}

// ── Account getters ──

pub fn get_account<T: AccountDeserialize>(svm: &LiteSVM, address: &Pubkey) -> T {
    let account = svm.get_account(address).expect("account not found");
    T::try_deserialize(&mut &account.data[..]).expect("failed to deserialize")
}

pub fn get_account_opt<T: AccountDeserialize>(svm: &LiteSVM, address: &Pubkey) -> Option<T> {
    let account = svm.get_account(address)?;
    if account.data.is_empty() {
        return None;
    }
    T::try_deserialize(&mut &account.data[..]).ok()
}

pub fn get_balance(svm: &LiteSVM, address: &Pubkey) -> u64 {
    svm.get_account(address).map(|a| a.lamports).unwrap_or(0)
}

pub fn account_exists(svm: &LiteSVM, address: &Pubkey) -> bool {
    svm.get_account(address).map(|a| !a.data.is_empty()).unwrap_or(false)
}

// ── Time helpers ──

pub const BASE_TS: i64 = 1_700_000_000;

pub fn future_ts(seconds_from_now: i64) -> i64 {
    BASE_TS + seconds_from_now
}

// ── Test scenario builders ──

pub struct SetupCtx {
    pub svm: LiteSVM,
    pub creator: Keypair,
    pub opponent: Keypair,
}

pub fn bootstrap_two_users() -> SetupCtx {
    let mut svm = setup();
    set_clock(&mut svm, BASE_TS);
    let creator = Keypair::new();
    let opponent = Keypair::new();
    fund_user(&mut svm, &creator);
    fund_user(&mut svm, &opponent);
    send_tx(&mut svm, &[ix_init_profile(&creator.pubkey())], &creator, &[&creator]).unwrap();
    send_tx(&mut svm, &[ix_init_profile(&opponent.pubkey())], &opponent, &[&opponent]).unwrap();
    SetupCtx { svm, creator, opponent }
}
