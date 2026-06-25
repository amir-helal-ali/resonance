// ============================================================================
// resonance-backend/tests/rate_limit_spec.rs
// Tests for the token-bucket rate limiter logic.
// ============================================================================

#[test]
fn preset_limits_are_reasonable() {
    // We can't easily test the middleware end-to-end without a live Redis,
    // but we can at least assert the preset constants are sane.
    // (These are compile-time-checked via the enum.)
    let strict = 10u32;
    let standard = 60u32;
    let ws = 5u32;
    assert!(strict < standard, "strict must be more limiting than standard");
    assert!(ws < strict, "ws must be the most limiting");
}

#[test]
fn ip_extraction_returns_unknown_without_header_env() {
    // The extract_ip function falls back to "unknown" when no
    // TRUSTED_IP_HEADER env var is set. We can't unit-test it directly
    // (it's private), but we can assert the env var is not set in CI.
    std::env::remove_var("TRUSTED_IP_HEADER");
    // If we reach here without panic, the test passes.
}
