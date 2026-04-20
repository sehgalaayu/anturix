mod helpers;
use helpers::*;
use solana_sdk::signature::{Keypair, Signer};
use anturix::state::{DuelState, Side};

const STAKE: u64 = 200_000_000;
const EXPIRY_OFFSET: i64 = 3600;

struct PreparedDuel {
    svm: LiteSVM,
    duel: solana_sdk::pubkey::Pubkey,
    resolver: Keypair,
}

fn create_and_match(
    condition: Condition,
    target_price: i64,
    lower_bound: i64,
    upper_bound: i64,
    price_feed_id_b: [u8; 32],
    remaining: &[solana_sdk::pubkey::Pubkey],
) -> PreparedDuel {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();

    let ix = ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, condition, SOL_USD_FEED, price_feed_id_b,
        target_price, lower_bound, upper_bound, future_ts(EXPIRY_OFFSET),
        remaining,
    );
    send_tx(&mut svm, &[ix], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    PreparedDuel { svm, duel, resolver }
}

fn resolve(d: &mut PreparedDuel, settlement_price: i64, remaining: &[solana_sdk::pubkey::Pubkey]) {
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut d.svm, new_ts);
    let price = create_mock_pyth_account(&mut d.svm, &SOL_USD_FEED, settlement_price, 1_000_000, -8, new_ts);
    send_tx(&mut d.svm, &[ix_resolve_duel(&d.resolver.pubkey(), &d.duel, &price, remaining)], &d.resolver, &[&d.resolver]).unwrap();
}

#[test]
fn condition_above_option_a_wins() {
    let mut d = create_and_match(Condition::Above, 100 * 100_000_000, 0, 0, [0u8; 32], &[]);
    resolve(&mut d, 150 * 100_000_000, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionA));
}

#[test]
fn condition_above_option_b_wins() {
    let mut d = create_and_match(Condition::Above, 100 * 100_000_000, 0, 0, [0u8; 32], &[]);
    resolve(&mut d, 50 * 100_000_000, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionB));
}

#[test]
fn condition_below() {
    let mut d = create_and_match(Condition::Below, 100 * 100_000_000, 0, 0, [0u8; 32], &[]);
    resolve(&mut d, 50 * 100_000_000, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionA));
}

#[test]
fn condition_odd() {
    // Price ends in 7 → odd → OPTION_A wins
    let mut d = create_and_match(Condition::Odd, 0, 0, 0, [0u8; 32], &[]);
    resolve(&mut d, 15_000_000_007, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionA));
}

#[test]
fn condition_even() {
    let mut d = create_and_match(Condition::Even, 0, 0, 0, [0u8; 32], &[]);
    resolve(&mut d, 15_000_000_008, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionA));
}

#[test]
fn condition_in_range() {
    let mut d = create_and_match(Condition::InRange, 0, 50 * 100_000_000, 150 * 100_000_000, [0u8; 32], &[]);
    resolve(&mut d, 100 * 100_000_000, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionA));
}

#[test]
fn condition_out_of_range() {
    let mut d = create_and_match(Condition::OutOfRange, 0, 50 * 100_000_000, 150 * 100_000_000, [0u8; 32], &[]);
    resolve(&mut d, 200 * 100_000_000, &[]);
    let duel: DuelState = get_account(&d.svm, &d.duel);
    assert_eq!(duel.winner_side, Some(Side::OptionA));
}

#[test]
fn condition_asset_race_a_outperforms() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();

    // Snapshot start prices at create time
    let price_a_start = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 100 * 100_000_000, 1_000_000, -8, BASE_TS);
    let price_b_start = create_mock_pyth_account(&mut svm, &BTC_USD_FEED, 60_000 * 100_000_000, 1_000_000, -8, BASE_TS);

    let ix = ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::AssetRace, SOL_USD_FEED, BTC_USD_FEED,
        0, 0, 0, future_ts(EXPIRY_OFFSET),
        &[price_a_start, price_b_start],
    );
    send_tx(&mut svm, &[ix], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    // After expiry: SOL up 50%, BTC up 10% → SOL outperforms → OPTION_A wins
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price_a_end = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, new_ts);
    let price_b_end = create_mock_pyth_account(&mut svm, &BTC_USD_FEED, 66_000 * 100_000_000, 1_000_000, -8, new_ts);

    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price_a_end, &[price_b_end])], &resolver, &[&resolver]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.winner_side, Some(Side::OptionA));
}
