mod helpers;
use helpers::*;
use solana_sdk::signature::{Keypair, Signer};
use anturix::state::Side;

const STAKE: u64 = 500_000_000;
const EXPIRY_OFFSET: i64 = 3600;

fn create_ix(creator: &Keypair, stake: u64, feed_id: [u8; 32], expires: i64) -> solana_sdk::instruction::Instruction {
    ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        stake, Condition::Above, feed_id, [0u8; 32],
        100 * 100_000_000, 0, 0, expires, &[],
    )
}

#[test]
fn create_rejects_stake_below_min() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();
    let err = send_tx_expect_err(&mut svm, &[create_ix(&creator, 10_000_000, SOL_USD_FEED, future_ts(EXPIRY_OFFSET))], &creator, &[&creator]);
    assert!(err.contains("StakeTooLow") || err.contains("6000"), "got: {}", err);
}

#[test]
fn create_rejects_expires_too_soon() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();
    // expires_at only 30 seconds from now (< 60s minimum)
    let err = send_tx_expect_err(&mut svm, &[create_ix(&creator, STAKE, SOL_USD_FEED, future_ts(30))], &creator, &[&creator]);
    assert!(err.contains("InvalidExpiry") || err.contains("6012"), "got: {}", err);
}

#[test]
fn create_rejects_zero_feed_id() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();
    let err = send_tx_expect_err(&mut svm, &[create_ix(&creator, STAKE, [0u8; 32], future_ts(EXPIRY_OFFSET))], &creator, &[&creator]);
    assert!(err.contains("InvalidFeedId") || err.contains("6023"), "got: {}", err);
}

#[test]
fn join_public_rejects_stake_below_min() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    let err = send_tx_expect_err(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, 1_000_000)], &opponent, &[&opponent]);
    assert!(err.contains("StakeTooLow") || err.contains("6000"), "got: {}", err);
}

#[test]
fn join_private_rejects_wrong_amount() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    let err = send_tx_expect_err(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE - 1)], &opponent, &[&opponent]);
    assert!(err.contains("ExactStakeRequired") || err.contains("6003"), "got: {}", err);
}

#[test]
fn join_private_rejects_same_side_as_creator() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    let err = send_tx_expect_err(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionA, STAKE)], &opponent, &[&opponent]);
    assert!(err.contains("WrongSide") || err.contains("6005"), "got: {}", err);
}

#[test]
fn join_private_rejects_double_match() {
    let mut svm = setup();
    set_clock(&mut svm, BASE_TS);
    let creator = Keypair::new();
    let opp1 = Keypair::new();
    let opp2 = Keypair::new();
    for kp in [&creator, &opp1, &opp2] {
        fund_user(&mut svm, kp);
        send_tx(&mut svm, &[ix_init_profile(&kp.pubkey())], kp, &[kp]).unwrap();
    }
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    send_tx(&mut svm, &[ix_join_pool(&opp1.pubkey(), &duel, Side::OptionB, STAKE)], &opp1, &[&opp1]).unwrap();

    // Second opponent attempts to join — already matched
    let err = send_tx_expect_err(&mut svm, &[ix_join_pool(&opp2.pubkey(), &duel, Side::OptionB, STAKE)], &opp2, &[&opp2]);
    assert!(err.contains("PrivateAlreadyMatched") || err.contains("6004"), "got: {}", err);
}

#[test]
fn join_rejects_creator_self_bet() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    let err = send_tx_expect_err(&mut svm, &[ix_join_pool(&creator.pubkey(), &duel, Side::OptionB, 50_000_000)], &creator, &[&creator]);
    assert!(err.contains("SelfDuel") || err.contains("6007"), "got: {}", err);
}

#[test]
fn resolve_rejects_before_expiry() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    // Clock still at BASE_TS — duel not expired
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, BASE_TS);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    let err = send_tx_expect_err(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]);
    assert!(err.contains("DuelNotExpired") || err.contains("6002"), "got: {}", err);
}

#[test]
fn resolve_rejects_price_before_expiry() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    // Price published BEFORE expires_at (20s before)
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, future_ts(EXPIRY_OFFSET) - 20);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    let err = send_tx_expect_err(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]);
    assert!(err.contains("PriceBeforeExpiry") || err.contains("6022"), "got: {}", err);
}

#[test]
fn resolve_rejects_high_confidence() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    // Confidence > 5% of price
    let price_val = 150 * 100_000_000;
    let conf = price_val as u64 / 10; // 10% > 5% threshold
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, price_val, conf, -8, new_ts);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    let err = send_tx_expect_err(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]);
    assert!(err.contains("PriceConfidenceTooWide") || err.contains("6019"), "got: {}", err);
}

#[test]
fn claim_share_rejects_before_resolve() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    // Status still Open — claim fails
    let err = send_tx_expect_err(&mut svm, &[ix_claim_share(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]);
    assert!(err.contains("InvalidDuelStatus") || err.contains("6001"), "got: {}", err);
}

#[test]
fn cancel_rejected_by_non_creator() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    let err = send_tx_expect_err(&mut svm, &[ix_cancel_duel(&opponent.pubkey(), &duel)], &opponent, &[&opponent]);
    assert!(err.contains("InvalidDuelStatus") || err.contains("6001"), "got: {}", err);
}
