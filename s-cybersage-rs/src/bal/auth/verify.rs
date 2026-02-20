use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::dal::dao::payment_dao::PaymentDao;

const MAX_AGE_SECONDS: i64 = 300;
const MAX_FUTURE_SKEW: i64 = 30;

pub struct AuthManager {
    payment_dao: PaymentDao,
}

impl AuthManager {
    pub fn new(payment_dao: PaymentDao) -> Self {
        Self { payment_dao }
    }

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

        let ts: i64 = timestamp
            .parse()
            .context("X-Signature-Timestamp is not a valid integer")?;

        let now: i64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        if ts > now + MAX_FUTURE_SKEW {
            bail!("Request timestamp is too far in the future");
        }

        if now - ts > MAX_AGE_SECONDS {
            bail!("Request timestamp is too old");
        }

        let public_key_bytes =
            hex::decode(public_key_hex).context("Failed to decode public key hex")?;

        let public_key_array: &[u8; 32] = public_key_bytes
            .as_slice()
            .try_into()
            .context("Public key has invalid length")?;

        let signature_bytes =
            hex::decode(signature_hex).context("Failed to decode signature hex")?;

        let signature_array: &[u8; 64] = signature_bytes
            .as_slice()
            .try_into()
            .context("Signature has invalid length")?;

        let public_key =
            VerifyingKey::from_bytes(public_key_array).context("Invalid public key bytes")?;

        let signature = Signature::from_bytes(signature_array);

        let mut message = Vec::with_capacity(timestamp.len() + body.len());
        message.extend_from_slice(timestamp.as_bytes());
        message.extend_from_slice(body);

        public_key
            .verify(&message, &signature)
            .context("Signature verification failed")?;

        Ok(())
    }

    pub async fn verify_subscription(&self, guild_id: &str) -> Result<()> {
        let is_active = self.payment_dao.is_active(guild_id).await?;

        if !is_active {
            bail!("Guild subscription is not active");
        }

        Ok(())
    }
}
