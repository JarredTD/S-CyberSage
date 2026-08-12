use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum accepted age for a signed Discord request.
const MAX_AGE_SECONDS: i64 = 300;
/// Maximum allowed future clock skew for a signed Discord request.
const MAX_FUTURE_SKEW: i64 = 30;

/// Verifies signed Discord interaction requests.
pub struct AuthManager {
    /// Prevents external construction while retaining a stable public constructor.
    _private: (),
}

impl Default for AuthManager {
    /// Creates a signature verifier with default validation behavior.
    fn default() -> Self {
        Self { _private: () }
    }
}

impl AuthManager {
    /// Creates a signature verifier.
    pub fn new() -> Self {
        Self::default()
    }

    /// Verifies the request timestamp and Ed25519 signature against the raw body.
    ///
    /// # Arguments
    ///
    /// * `signature_hex` - Value of Discord's Ed25519 signature header.
    /// * `timestamp` - Value of Discord's request timestamp header.
    /// * `body` - Exact raw request body received from Discord.
    /// * `public_key_hex` - Discord application public key encoded as hexadecimal.
    ///
    /// # Errors
    ///
    /// Returns an error when headers are missing, the timestamp is outside the accepted window,
    /// key material is malformed, or the signature does not match the request body.
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

/// Tests Discord request signature verification without external dependencies.
#[cfg(test)]
mod tests {
    use super::AuthManager;
    use ed25519_dalek::{Signer, SigningKey};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Verifies a correctly signed current request.
    #[test]
    fn accepts_valid_signature() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let body = br#"{\"type\":1}"#;
        let timestamp = current_timestamp();
        let message = [timestamp.as_bytes(), body].concat();
        let signature = signing_key.sign(&message);

        let result = AuthManager::new().verify_signature(
            &hex::encode(signature.to_bytes()),
            &timestamp,
            body,
            &hex::encode(signing_key.verifying_key().to_bytes()),
        );

        assert!(result.is_ok());
    }

    /// Rejects a request with a missing signature header.
    #[test]
    fn rejects_missing_signature() {
        let result = AuthManager::new().verify_signature("", "1", b"{}", "00");

        assert!(result.is_err());
    }

    /// Confirms that the default verifier has the same behavior as an explicit constructor.
    #[test]
    fn default_creates_verifier() {
        let result = AuthManager::default().verify_signature("", "1", b"{}", "00");

        assert!(result.is_err());
    }

    /// Returns a timestamp that falls within the verifier's accepted time window.
    fn current_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_secs()
            .to_string()
    }
}
