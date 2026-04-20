mod helpers;
use helpers::*;
use solana_sdk::signature::{Keypair, Signer};
use anturix::state::{DuelState, DuelStatus, Position, UserProfile};

const STAKE: u64 = 500_000_000; // 0.5 SOL
const EXPIRY_OFFSET: i64 = 3600;

fn default_create_duel(
    creator: &Keypair,
    duel_count: u64,
    visibility: Visibility,
    creator_side: Side,
) -> solana_sdk::instruction::Instruction {
    ix_create_duel(
        &creator.pubkey(),
        duel_count,
        visibility,
        creator_side,
        STAKE,
        Condition::Above,
        SOL_USD_FEED,
        [0u8; 32],
        100 * 100_000_000, // target_price $100
        0,
        0,
        future_ts(EXPIRY_OFFSET),
        &[],
    )
}

#[test]
fn private_mode_creator_wins() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();

    // Create: Private, creator on OPTION_A, condition Above target
    send_tx(&mut svm, &[default_create_duel(&creator, 0, Visibility::Private, Side::OptionA)], &creator, &[&creator]).unwrap();

    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    // Opponent joins OPTION_B with matching stake
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    // Advance past expiry with price ABOVE target ($150 > $100) — OPTION_A wins
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, new_ts);

    // Resolve
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.status, DuelStatus::Resolved);
    assert_eq!(duel_acc.winner_side, Some(Side::OptionA));

    // Creator (OPTION_A) claims
    let creator_bal_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let creator_bal_after = get_balance(&svm, &creator.pubkey());

    // Creator gets total_pot (2 × STAKE) minus tx fees + Position rent back
    assert!(creator_bal_after > creator_bal_before + STAKE);

    // Position closed
    let (cpos, _) = position_pda(&duel, &creator.pubkey(), Side::OptionA);
    assert!(!account_exists(&svm, &cpos));

    // Opponent (loser) tries to claim — should fail
    let err = send_tx_expect_err(&mut svm, &[ix_claim_share(&opponent.pubkey(), &duel, Side::OptionB)], &opponent, &[&opponent]);
    assert!(err.contains("NotOnWinningSide") || err.contains("6009"));

    // Creator profile wins incremented
    let (cprof, _) = profile_pda(&creator.pubkey());
    let profile: UserProfile = get_account(&svm, &cprof);
    assert_eq!(profile.wins, 1);
}

#[test]
fn private_mode_opponent_wins() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();

    send_tx(&mut svm, &[default_create_duel(&creator, 0, Visibility::Private, Side::OptionA)], &creator, &[&creator]).unwrap();

    let (duel, _) = duel_pda(&creator.pubkey(), 0);
    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    // Price BELOW target — OPTION_A loses, OPTION_B wins
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 50 * 100_000_000, 1_000_000, -8, new_ts);

    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.winner_side, Some(Side::OptionB));

    let opp_bal_before = get_balance(&svm, &opponent.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&opponent.pubkey(), &duel, Side::OptionB)], &opponent, &[&opponent]).unwrap();
    let opp_bal_after = get_balance(&svm, &opponent.pubkey());
    assert!(opp_bal_after > opp_bal_before + STAKE);
}

#[test]
fn public_mode_multi_bettor_payout() {
    let mut svm = setup();
    set_clock(&mut svm, BASE_TS);

    let creator = Keypair::new();
    let b1 = Keypair::new();
    let b2 = Keypair::new();
    let b3 = Keypair::new();
    for kp in [&creator, &b1, &b2, &b3] {
        fund_user(&mut svm, kp);
        send_tx(&mut svm, &[ix_init_profile(&kp.pubkey())], kp, &[kp]).unwrap();
    }

    // Creator stakes 1 SOL on OPTION_A
    let create_stake = 1_000_000_000;
    let ix = ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        create_stake, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    );
    send_tx(&mut svm, &[ix], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    // b1 also on OPTION_A with 2 SOL. b2 on OPTION_B with 1 SOL. b3 on OPTION_B with 2 SOL.
    // Total side_a = 3 SOL, side_b = 3 SOL, pot = 6 SOL
    send_tx(&mut svm, &[ix_join_pool(&b1.pubkey(), &duel, Side::OptionA, 2_000_000_000)], &b1, &[&b1]).unwrap();
    send_tx(&mut svm, &[ix_join_pool(&b2.pubkey(), &duel, Side::OptionB, 1_000_000_000)], &b2, &[&b2]).unwrap();
    send_tx(&mut svm, &[ix_join_pool(&b3.pubkey(), &duel, Side::OptionB, 2_000_000_000)], &b3, &[&b3]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.side_a_total, 3_000_000_000);
    assert_eq!(duel_acc.side_b_total, 3_000_000_000);

    // Advance + resolve with OPTION_A winning (price > target)
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 200 * 100_000_000, 1_000_000, -8, new_ts);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    // Creator on OPTION_A (stake 1, winning_total 3, pot 6) → share = 6 * 1 / 3 = 2 SOL
    let creator_bal_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let creator_bal_after = get_balance(&svm, &creator.pubkey());
    assert!(creator_bal_after > creator_bal_before + 1_900_000_000, "creator should get ~2 SOL, got delta {}", creator_bal_after - creator_bal_before);

    // b1 on OPTION_A (stake 2, winning_total 3, pot 6) → share = 4 SOL
    let b1_bal_before = get_balance(&svm, &b1.pubkey());
    send_tx(&mut svm, &[ix_claim_share(&b1.pubkey(), &duel, Side::OptionA)], &b1, &[&b1]).unwrap();
    let b1_bal_after = get_balance(&svm, &b1.pubkey());
    assert!(b1_bal_after > b1_bal_before + 3_900_000_000, "b1 should get ~4 SOL");

    // b2 (loser) claim fails
    let err = send_tx_expect_err(&mut svm, &[ix_claim_share(&b2.pubkey(), &duel, Side::OptionB)], &b2, &[&b2]);
    assert!(err.contains("NotOnWinningSide") || err.contains("6009"));
}

#[test]
fn public_mode_top_up_same_side() {
    let mut svm = setup();
    set_clock(&mut svm, BASE_TS);
    let creator = Keypair::new();
    let bettor = Keypair::new();
    for kp in [&creator, &bettor] {
        fund_user(&mut svm, kp);
        send_tx(&mut svm, &[ix_init_profile(&kp.pubkey())], kp, &[kp]).unwrap();
    }

    let ix = ix_create_duel(
        &creator.pubkey(), 0, Visibility::Public, Side::OptionA,
        STAKE, Condition::Above, SOL_USD_FEED, [0u8; 32],
        100 * 100_000_000, 0, 0, future_ts(EXPIRY_OFFSET), &[],
    );
    send_tx(&mut svm, &[ix], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    // Bettor joins OPTION_B with 0.1 SOL, then tops up 0.2 SOL more
    send_tx(&mut svm, &[ix_join_pool(&bettor.pubkey(), &duel, Side::OptionB, 100_000_000)], &bettor, &[&bettor]).unwrap();
    send_tx(&mut svm, &[ix_join_pool(&bettor.pubkey(), &duel, Side::OptionB, 200_000_000)], &bettor, &[&bettor]).unwrap();

    let (pos, _) = position_pda(&duel, &bettor.pubkey(), Side::OptionB);
    let position: Position = get_account(&svm, &pos);
    assert_eq!(position.stake, 300_000_000);

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.side_b_total, 300_000_000);
}

#[test]
fn orphan_auto_cancel_on_resolve() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();

    send_tx(&mut svm, &[default_create_duel(&creator, 0, Visibility::Public, Side::OptionA)], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    // Past expiry, no one joined. Resolve should auto-cancel.
    let new_ts = future_ts(EXPIRY_OFFSET + 1);
    set_clock(&mut svm, new_ts);
    let price = create_mock_pyth_account(&mut svm, &SOL_USD_FEED, 150 * 100_000_000, 1_000_000, -8, new_ts);
    let resolver = Keypair::new();
    fund_user(&mut svm, &resolver);
    send_tx(&mut svm, &[ix_resolve_duel(&resolver.pubkey(), &duel, &price, &[])], &resolver, &[&resolver]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.status, DuelStatus::Cancelled);
    assert_eq!(duel_acc.winner_side, None);

    // Creator can refund
    let bal_before = get_balance(&svm, &creator.pubkey());
    send_tx(&mut svm, &[ix_claim_refund(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
    let bal_after = get_balance(&svm, &creator.pubkey());
    assert!(bal_after > bal_before + STAKE - 100_000); // minus tx fee
}

#[test]
fn creator_cancel_allowed_when_opposite_empty() {
    let SetupCtx { mut svm, creator, .. } = bootstrap_two_users();

    send_tx(&mut svm, &[default_create_duel(&creator, 0, Visibility::Private, Side::OptionA)], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    send_tx(&mut svm, &[ix_cancel_duel(&creator.pubkey(), &duel)], &creator, &[&creator]).unwrap();

    let duel_acc: DuelState = get_account(&svm, &duel);
    assert_eq!(duel_acc.status, DuelStatus::Cancelled);

    // Creator refunds
    send_tx(&mut svm, &[ix_claim_refund(&creator.pubkey(), &duel, Side::OptionA)], &creator, &[&creator]).unwrap();
}

#[test]
fn creator_cancel_rejected_when_opposite_has_liquidity() {
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();

    send_tx(&mut svm, &[default_create_duel(&creator, 0, Visibility::Private, Side::OptionA)], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    send_tx(&mut svm, &[ix_join_pool(&opponent.pubkey(), &duel, Side::OptionB, STAKE)], &opponent, &[&opponent]).unwrap();

    let err = send_tx_expect_err(&mut svm, &[ix_cancel_duel(&creator.pubkey(), &duel)], &creator, &[&creator]);
    assert!(err.contains("OppositeSideNotEmpty") || err.contains("6011"));
}

#[test]
fn force_cancel_rejected_for_non_admin() {
    // Admin positive path tested on devnet with real admin keypair.
    // Here we assert the unauthorized path: non-admin cannot force-cancel.
    let SetupCtx { mut svm, creator, opponent } = bootstrap_two_users();

    send_tx(&mut svm, &[default_create_duel(&creator, 0, Visibility::Public, Side::OptionA)], &creator, &[&creator]).unwrap();
    let (duel, _) = duel_pda(&creator.pubkey(), 0);

    let err = send_tx_expect_err(&mut svm, &[ix_force_cancel_duel(&opponent.pubkey(), &duel)], &opponent, &[&opponent]);
    assert!(err.contains("UnauthorizedAdmin") || err.contains("6008"), "got: {}", err);
}
