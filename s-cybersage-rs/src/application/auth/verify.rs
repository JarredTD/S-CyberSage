use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum accepted age for a signed Discord request.
const MAX_AGE_SECONDS: i64 = 300;
/// Maximum allowed future clock skew for a signed Discord request.
const MAX_FUTURE_SKEW: i64 = 30;

/// Verifies signed Discord interaction requests.
pub struct AuthManager;

impl AuthManager {
    /// Creates a signature verifier.
    pub fn new() -> Self {
        Self
    }

    /// Verifies the request timestamp and Ed25519 signature against the raw body.
    pub fn verify_signature(
        &self,
        signature_hex: &str,
        timestamp: &str,
        body: &[u8],
        public_key_hex: &str,
    ) -> Result<()> {
        if signature_hex.is_empty() || timestamp.is_empty() {
            bail!("Missing required Discord signature headers");
        }

        let ts: i64 = timestamp.parse().context("Invalid signature timestamp")?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        if ts > now + MAX_FUTURE_SKEW {
            bail!("Request timestamp too far in future");
        }

        if now - ts > MAX_AGE_SECONDS {
            bail!("Request timestamp too old");
        }

        let public_key_bytes = hex::decode(public_key_hex).context("Invalid public key hex")?;

        let public_key_array: &[u8; 32] = public_key_bytes
            .as_slice()
            .try_into()
            .context("Public key must be 32 bytes")?;

        let public_key =
            VerifyingKey::from_bytes(public_key_array).context("Invalid public key")?;

        let signature_bytes = hex::decode(signature_hex).context("Invalid signature hex")?;

        let signature_array: &[u8; 64] = signature_bytes
            .as_slice()
            .try_into()
            .context("Signature must be 64 bytes")?;

        let signature = Signature::from_bytes(signature_array);

        let mut message = Vec::with_capacity(timestamp.len() + body.len());
        message.extend_from_slice(timestamp.as_bytes());
        message.extend_from_slice(body);

        public_key
            .verify(&message, &signature)
            .context("Signature verification failed")?;

        Ok(())
    }
}
