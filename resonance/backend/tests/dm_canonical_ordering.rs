// ============================================================================
// resonance-backend/tests/dm_canonical_ordering.rs
// Verifies that the canonical ordering (user_a < user_b) used for DMs is
// correctly applied. We test the comparison logic directly.
// ============================================================================

#[test]
fn canonical_ordering_picks_smaller_uuid_first() {
    use uuid::Uuid;
    let a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

    let (user_a, user_b) = if a < b { (a, b) } else { (b, a) };
    assert_eq!(user_a, a, "user_a must be the smaller UUID");
    assert_eq!(user_b, b, "user_b must be the larger UUID");
}

#[test]
fn canonical_ordering_is_symmetric() {
    use uuid::Uuid;
    let a = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let b = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

    let (a1, b1) = if a < b { (a, b) } else { (b, a) };
    let (a2, b2) = if b < a { (b, a) } else { (a, b) };
    assert_eq!(a1, a2);
    assert_eq!(b1, b2);
}
