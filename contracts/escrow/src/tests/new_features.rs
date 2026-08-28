/// Tests for issues #1334, #1335, #1336, #1337.
use super::*;
use soroban_sdk::testutils::Address as _;

// ── Issue #1336: get_contract_version ────────────────────────────────────────

#[test]
fn test_get_contract_version_returns_semver_string() {
    let (env, contract_id, ..) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);
    let version = client.get_contract_version();
    assert_eq!(version, String::from_str(&env, "0.1.0"));
}

// ── Issue #1337: protocol fee cap ────────────────────────────────────────────

#[test]
fn test_protocol_fee_no_cap_takes_full_percentage() {
    let (env, contract_id, oracle, player1, player2, token, admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&player1, &900);
    asset_client.mint(&player2, &900);

    // 10% fee, no cap
    client.set_protocol_fee_bps(&1000);

    let match_id = client.create_match(
        &player1,
        &player2,
        &1000,
        &token,
        &String::from_str(&env, "fee_no_cap"),
        &Platform::Lichess,
        &None,
    );
    client.deposit(&match_id, &player1);
    client.deposit(&match_id, &player2);

    let tc = TokenClient::new(&env, &token);
    let p1_before = tc.balance(&player1);

    client.submit_result(&match_id, &Winner::Player1);

    // pot=2000, fee=200 (10%), winner gets 1800
    let p1_after = tc.balance(&player1);
    assert_eq!(p1_after - p1_before, 1800);
    // admin received 200
    assert_eq!(tc.balance(&admin), 200);
}

#[test]
fn test_protocol_fee_cap_limits_fee() {
    let (env, contract_id, oracle, player1, player2, token, admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&player1, &900);
    asset_client.mint(&player2, &900);

    // 10% fee with cap of 50
    client.set_protocol_fee_bps(&1000);
    client.set_max_protocol_fee(&Some(50));

    let match_id = client.create_match(
        &player1,
        &player2,
        &1000,
        &token,
        &String::from_str(&env, "fee_with_cap"),
        &Platform::Lichess,
        &None,
    );
    client.deposit(&match_id, &player1);
    client.deposit(&match_id, &player2);

    let tc = TokenClient::new(&env, &token);
    let p1_before = tc.balance(&player1);

    client.submit_result(&match_id, &Winner::Player1);

    // pot=2000, calculated_fee=200 but capped at 50, winner gets 1950
    let p1_after = tc.balance(&player1);
    assert_eq!(p1_after - p1_before, 1950);
    assert_eq!(tc.balance(&admin), 50);
}

#[test]
fn test_protocol_fee_zero_no_deduction() {
    let (env, contract_id, _oracle, player1, player2, token, _admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);

    let match_id = client.create_match(
        &player1,
        &player2,
        &100,
        &token,
        &String::from_str(&env, "fee_zero"),
        &Platform::Lichess,
        &None,
    );
    client.deposit(&match_id, &player1);
    client.deposit(&match_id, &player2);

    let tc = TokenClient::new(&env, &token);
    let p1_before = tc.balance(&player1);
    client.submit_result(&match_id, &Winner::Player1);
    // no fee: winner gets full pot of 200
    assert_eq!(tc.balance(&player1) - p1_before, 200);
}

// ── Issue #1334: referral tracking ───────────────────────────────────────────

#[test]
fn test_referrer_receives_fee_on_payout() {
    let (env, contract_id, _oracle, player1, player2, token, _admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&player1, &900);
    asset_client.mint(&player2, &900);

    let referrer = Address::generate(&env);

    // 10% protocol fee, 50% of that goes to referrer
    client.set_protocol_fee_bps(&1000);
    client.set_referral_share_bps(&5000);

    let match_id = client.create_match(
        &player1,
        &player2,
        &1000,
        &token,
        &String::from_str(&env, "referral_test"),
        &Platform::Lichess,
        &Some(referrer.clone()),
    );
    client.deposit(&match_id, &player1);
    client.deposit(&match_id, &player2);

    let tc = TokenClient::new(&env, &token);
    let referrer_before = tc.balance(&referrer);

    client.submit_result(&match_id, &Winner::Player1);

    // pot=2000, fee=200, referral=100 (50% of 200), winner gets 1800
    let referrer_after = tc.balance(&referrer);
    assert_eq!(referrer_after - referrer_before, 100);
}

#[test]
fn test_no_referrer_full_fee_goes_to_admin() {
    let (env, contract_id, _oracle, player1, player2, token, admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);
    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&player1, &900);
    asset_client.mint(&player2, &900);

    client.set_protocol_fee_bps(&1000);
    client.set_referral_share_bps(&5000);

    let match_id = client.create_match(
        &player1,
        &player2,
        &1000,
        &token,
        &String::from_str(&env, "no_referrer_test"),
        &Platform::Lichess,
        &None,
    );
    client.deposit(&match_id, &player1);
    client.deposit(&match_id, &player2);

    let tc = TokenClient::new(&env, &token);
    let admin_before = tc.balance(&admin);

    client.submit_result(&match_id, &Winner::Player1);

    // No referrer: full 200 fee goes to admin
    assert_eq!(tc.balance(&admin) - admin_before, 200);
}

#[test]
fn test_referrer_stored_on_match() {
    let (env, contract_id, _oracle, player1, player2, token, _admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);

    let referrer = Address::generate(&env);
    let match_id = client.create_match(
        &player1,
        &player2,
        &100,
        &token,
        &String::from_str(&env, "referrer_stored"),
        &Platform::Lichess,
        &Some(referrer.clone()),
    );

    let m = client.get_match(&match_id);
    assert_eq!(m.referrer, Some(referrer));
}

// ── Issue #1335: deposit_batch ───────────────────────────────────────────────

#[test]
fn test_deposit_batch_valid_entries() {
    let (env, contract_id, _oracle, player1, player2, token, _admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);

    let match_id1 = client.create_match(
        &player1, &player2, &100, &token,
        &String::from_str(&env, "batch_1"), &Platform::Lichess, &None,
    );
    let match_id2 = client.create_match(
        &player1, &player2, &100, &token,
        &String::from_str(&env, "batch_2"), &Platform::Lichess, &None,
    );

    let entries: soroban_sdk::Vec<(u64, Address)> = soroban_sdk::vec![
        &env,
        (match_id1, player1.clone()),
        (match_id2, player1.clone()),
    ];

    let results = client.deposit_batch(&entries);
    assert_eq!(results.len(), 2);
    assert_eq!(results.get(0).unwrap(), 0u32);
    assert_eq!(results.get(1).unwrap(), 0u32);

    assert!(client.get_match(&match_id1).player1_deposited);
    assert!(client.get_match(&match_id2).player1_deposited);
}

#[test]
fn test_deposit_batch_mixed_valid_invalid() {
    let (env, contract_id, _oracle, player1, player2, token, _admin) = setup();
    let client = EscrowContractClient::new(&env, &contract_id);

    let match_id = client.create_match(
        &player1, &player2, &100, &token,
        &String::from_str(&env, "batch_mixed"), &Platform::Lichess, &None,
    );

    // First entry valid, second is a non-existent match (999)
    let entries: soroban_sdk::Vec<(u64, Address)> = soroban_sdk::vec![
        &env,
        (match_id, player1.clone()),
        (999u64, player1.clone()),
    ];

    let results = client.deposit_batch(&entries);
    assert_eq!(results.len(), 2);
    // First succeeds
    assert_eq!(results.get(0).unwrap(), 0u32);
    // Second fails with MatchNotFound (error code 1)
    assert_eq!(results.get(1).unwrap(), Error::MatchNotFound as u32);
}
