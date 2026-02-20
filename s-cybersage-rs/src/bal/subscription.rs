use anyhow::{bail, Result};

use crate::dal::dao::payment_dao::PaymentDao;

const DEFAULT_SUBSCRIPTION_DURATION_SECONDS: i64 = 30 * 24 * 60 * 60;

#[derive(Clone)]
pub struct SubscriptionManager {
    payment_dao: PaymentDao,
}

impl SubscriptionManager {
    pub fn new(payment_dao: PaymentDao) -> Self {
        Self { payment_dao }
    }

    pub async fn subscribe(&self, guild_id: &str) -> Result<()> {
        if self.payment_dao.is_active(guild_id).await? {
            bail!("Guild already has an active subscription");
        }

        self.payment_dao
            .subscribe_guild(guild_id, DEFAULT_SUBSCRIPTION_DURATION_SECONDS)
            .await?;

        Ok(())
    }

    pub async fn unsubscribe(&self, guild_id: &str) -> Result<()> {
        self.payment_dao.cancel_subscription(guild_id).await?;

        Ok(())
    }

    pub async fn is_active(&self, guild_id: &str) -> Result<bool> {
        self.payment_dao.is_active(guild_id).await
    }
}
