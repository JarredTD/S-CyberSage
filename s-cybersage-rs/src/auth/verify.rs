use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::convert::TryInto;

pub fn verify_discord_request(
    signature_hex: &str,
    timestamp: &str,
    body: &[u8],
    public_key_hex: &str,
) -> Result<()> {
    let public_key_bytes =
        hex::decode(public_key_hex).context("Failed to decode public key hex")?;

    let public_key_array: &[u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .context("Public key has invalid length")?;

    let signature_bytes = hex::decode(signature_hex).context("Failed to decode signature hex")?;

    let signature_array: &[u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .context("Signature has invalid length")?;

    let public_key =
        VerifyingKey::from_bytes(public_key_array).context("Invalid public key bytes")?;

    let signature = Signature::from_bytes(signature_array);

    let mut message = timestamp.as_bytes().to_vec();
    message.extend_from_slice(body);

    public_key
        .verify(&message, &signature)
        .context("Signature verification failed")?;

    Ok(())
}
