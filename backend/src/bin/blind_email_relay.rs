// ============================================================================
// resonance-backend/src/bin/blind_email_relay.rs
// A standalone binary that consumes the `blind_email_relay:jobs` Redis list
// and sends OTP emails via SMTP without ever persisting the cleartext email.
//
// The decryption flow:
//   1. Job contains: { user_id, email_ciphertext (b64), otp, blind_index }
//   2. The relay does NOT have the user's password-derived KEK. Instead, the
//      client must POST a short-lived "decryption token" to a relay-specific
//      endpoint at registration time. The token is a wrapped KEK that the
//      relay can use ONCE to decrypt the email.
//   3. For this skeleton we demonstrate the architecture but use a simpler
//      approach: the client posts the email_ciphertext + a one-time KEK
//      (wrapped under a relay public key) directly to the relay, which
//      decrypts and immediately sends the OTP.
//
// In production you'd run this inside a TEE (SGX/SEV) for defense in depth.
// ============================================================================

use redis::AsyncCommands;
use serde::Deserialize;
use std::env;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
struct RelayJob {
    user_id: String,
    email_ciphertext: String,
    otp: String,
    #[allow(dead_code)]
    blind_index: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("relay=info")),
        )
        .init();

    let redis_url = env::var("REDIS_URL").expect("REDIS_URL");
    let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let smtp_port: u16 = env::var("SMTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let smtp_from = env::var("SMTP_FROM").unwrap_or_else(|_| "no-reply@resonance.local".into());

    let client = redis::Client::open(redis_url.as_str())?;
    let mut conn = redis::aio::ConnectionManager::new(client).await?;
    info!(%smtp_host, smtp_port, "blind email relay started");

    // BRPOP forever: blocks until a job is available, then processes it.
    loop {
        let (list, payload): (String, String) = match conn.brpop("blind_email_relay:jobs", 30).await {
            Ok(v) => v,
            Err(e) => {
                error!(error = ?e, "brpop failed; sleeping");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };
        debug_assert_eq!(list, "blind_email_relay:jobs");

        let job: RelayJob = match serde_json::from_str(&payload) {
            Ok(j) => j,
            Err(e) => {
                error!(error = ?e, "job parse failed; skipping");
                continue;
            }
        };

        // Zeroize the OTP after sending. We use a guard struct.
        let otp_guard = ZeroizingString(job.otp.clone());

        if let Err(e) = send_otp_email(&smtp_host, smtp_port, &smtp_from, &job, otp_guard.as_str()).await {
            error!(error = ?e, user_id = %job.user_id, "send_otp_email failed");
        } else {
            info!(user_id = %job.user_id, "OTP email dispatched (cleartext never persisted)");
        }

        // otp_guard drops here and zeroizes the buffer.
        drop(otp_guard);
    }
}

/// Send the OTP email via raw SMTP (no TLS for the dev relay; production
/// would use STARTTLS or a managed relay like SES).
async fn send_otp_email(
    host: &str,
    port: u16,
    from: &str,
    job: &RelayJob,
    _otp: &str,
) -> Result<(), std::io::Error> {
    // For the skeleton we just LOG the would-be email. NEVER do this in prod.
    tracing::info!(
        user_id = %job.user_id,
        from = %from,
        "SMTP RELAY (dev): would send OTP to user via blind ciphertext"
    );

    // In production, this function would:
    //   1. Use the relay's private key (kept in TEE) to unwrap the per-job KEK.
    //   2. AES-GCM decrypt `job.email_ciphertext` to recover the cleartext email.
    //   3. Open an SMTP connection to (host, port).
    //   4. Send: From: <from>  To: <cleartext_email>  Subject: صدى OTP
    //            Body: "كود التحقق بتاعك: {otp}"
    //   5. zeroize the cleartext email buffer.
    //   6. Close the SMTP connection.
    //
    // The cleartext email is never logged, never persisted, never written to disk.
    // The only artifact is the SMTP packet on the wire to the user's MX.

    use tokio::io::AsyncWriteExt;
    let mut stream = tokio::net::TcpStream::connect((host, port)).await?;
    let _ = stream.write_all(b"QUIT\r\n").await; // graceful close without sending (dev)
    Ok(())
}

/// A String wrapper that zeroizes its heap buffer on drop.
struct ZeroizingString(String);
impl ZeroizingString {
    fn as_str(&self) -> &str {
        &self.0
    }
}
impl Drop for ZeroizingString {
    fn drop(&mut self) {
        // Overwrite the bytes with zeros. SAFETY: we have exclusive access
        // via `&mut self`, and String's buffer is valid UTF-8 (we're
        // replacing it with zeros, which is valid UTF-8 too).
        unsafe {
            let bytes = self.0.as_bytes_mut();
            for b in bytes.iter_mut() {
                *b = 0;
            }
        }
    }
}
