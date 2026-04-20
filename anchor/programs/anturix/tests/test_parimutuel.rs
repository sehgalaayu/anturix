mod helpers;
use helpers::*;
use solana_sdk::signature::{Keypair, Signer};
use anturix::state::{DuelState, Side};

const EXPIRY_OFFSET: i64 = 3600;

/// In Private mode (1v1, equal stakes): winner gets exactly 2x their stake.
#[test]
fn private_mode_2x_payout() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();
    let stake = 1_000_000_000; // 1 SOL

    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Private, Side::OptionA,
        stake, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, stake)], &opponent, &[&opponent]).unwrap();

    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, new_ts);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    // Escrow should have ~2 SOL
    let (escrow, _) = escrow_pda(&duel);
    let escrow_bal = get_balance(&svm, &escrow);
    assert_eq!(escrow_bal, 2 * stake);

    let bal_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let bal_after = get_balance(&svm, &creator.pubkey());

    // Delta ~ 2 * stake (plus position rent back, minus tx fee)
    let delta = bal_after - bal_before;
    assert!(delta > 2 * stake - 100_000, "delta {} should be ~= {}", delta, 2 * stake);
    assert!(delta < 2 * stake + 10_000_000, "delta {} should be ~= {}", delta, 2 * stake);
}

/// Public mode with unbalanced pool: winning side's payout includes losing side's stake proportionally.
/// Setup: side_a = 100, side_b = 400. Total pot = 500. If A wins: each A-bettor gets 5x their stake.
#[test]
fn public_mode_unbalanced_pool_payout() {
    let mut svm = setup();
    set_clock(&mut svm, BASE_TS);

    let creator = Keypair::new(); // side A, 100M lamports
    let a2 = Keypair::new();       // side A bettor — won't exist in this test (only creator on A)
    let b1 = Keypair::new();       // side B, 200M
    let b2 = Keypair::new();       // side B, 200M
    for kp in [&creator, &a2, &b1, &b2] {
        fund_user(&mut svm, kp);
        send_tx(&mut svm, &[ix_init_profile(&kp.pubkey())], kp, &[kp]).unwrap();
    }

    // Creator stakes 100M on OPTION_A
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        100_000_000, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    // b1 + b2 on OPTION_B with 200M each — side_b = 400M
    send_tx(&mut svm, &[ix_join_pool(&b1.pubkey(), &duel, Side::OptionB, 200_000_000)], &b1, &[&b1]).unwrap();
    send_tx(&mut svm, &[ix_join_pool(&b2.pubkey(), &duel, Side::OptionB, 200_000_000)], &b2, &[&b2]).unwrap();

    // Resolve OPTION_A wins
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, new_ts);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.side_a_total, 100_000_000);
    assert_eq!(duel_acc.side_b_total, 400_000_000);

    // Creator share: total_pot × stake / winning_total = 500M × 100M / 100M = 500M (5x payout)
    let bal_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let bal_after = get_balance(&svm, &creator.pubkey());
    let delta = bal_after - bal_before;
    assert!(delta > 499_000_000, "delta {} should be ~500M (5x)", delta);
    assert!(delta < 510_000_000, "delta {} should be ~500M (5x)", delta);
}

/// Two winners split the pot proportionally.
#[test]
fn public_mode_proportional_split_among_winners() {
    let mut svm = setup();
    set_clock(&mut svm, BASE_TS);

    let creator = Keypair::new();
    let a2 = Keypair::new();
    let b1 = Keypair::new();
    for kp in [&creator, &a2, &b1] {
        fund_user(&mut svm, kp);
        send_tx(&mut svm, &[ix_init_profile(&kp.pubkey())], kp, &[kp]).unwrap();
    }

    // Creator: 100M on A. a2: 300M on A (total_a = 400M). b1: 200M on B. Pot = 600M.
    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        100_000_000, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&a2.pubkey(), &duel, Side::OptionA, 300_000_000)], &a2, &[&a2]).unwrap();
    send_tx(&mut svm, &[ix_join_pool(&b1.pubkey(), &duel, Side::OptionB, 200_000_000)], &b1, &[&b1]).unwrap();

    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, new_ts);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    // Creator: 600M × 100M / 400M = 150M (1.5x)
    let c_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let c_after = get_balance(&svm, &creator.pubkey());
    let c_delta = c_after - c_before;
    assert!(c_delta > 149_000_000 && c_delta < 160_000_000, "creator delta {} should be ~150M", c_delta);

    // a2: 600M × 300M / 400M = 450M (1.5x)
    let a2_before = get_balance(&svm, &a2.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&a2.pubkey(), &duel, Side::OptionA)], &a2, &[&a2]).unwrap();
    let a2_after = get_balance(&svm, &a2.pubkey());
    let a2_delta = a2_after - a2_before;
    assert!(a2_delta > 449_000_000 && a2_delta < 460_000_000, "a2 delta {} should be ~450M", a2_delta);

    // Escrow should be near-empty (modulo dust)
    let (escrow, _) = escrow_pda(&duel);
    let escrow_bal = get_balance(&svm, &escrow);
    assert!(escrow_bal < 1000, "escrow should be drained, has {}", escrow_bal);
}

#[test]
fn refund_returns_full_stake() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();
    let stake = 500_000_000;

    send_tx(&mut svm, &[ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        stake, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    )], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    // Creator cancels (opposite empty)
    send_tx(&mut svm, &[ix_cancel_duel(&creator.pubkey(), &duel)], &creator, &[&creator]).unwrap();

    let bal_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_refund(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let bal_after = get_balance(&svm, &creator.pubkey());

    // Should get full stake + position rent back (minus tx fees)
    let delta = bal_after - bal_before;
    assert!(delta > stake - 100_000, "refund delta {} should be ~= stake {}", delta, stake);
}
