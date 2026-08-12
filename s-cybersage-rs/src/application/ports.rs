use anyhow::Result;

/// Represents a role that a guild permits members to self-assign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRegistration {
    /// Human-readable Discord role name.
    pub name: String,
    /// Stable Discord role identifier.
    pub id: String,
}

/// Describes a requested change to a member's role membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleMembershipAction {
    /// Adds the role to the member.
    Add,
    /// Removes the role from the member.
    Remove,
}

/// Reads and writes self-assignable role registrations.
#[allow(async_fn_in_trait)]
pub trait GuildRoleRepository {
    /// Returns saved roles whose names begin with `prefix`.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot complete the lookup.
    async fn query_roles_by_prefix(
        &self,
        guild_id: &str,
        prefix: &str,
    ) -> Result<Vec<RoleRegistration>>;

    /// Saves or replaces a role registration for `guild_id`.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot complete the write.
    async fn save_role(&self, guild_id: &str, role: &RoleRegistration) -> Result<()>;

    /// Finds a saved role by its case-insensitive display name.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot complete the lookup.
    async fn get_role_by_name(
        &self,
        guild_id: &str,
        role_name: &str,
    ) -> Result<Option<RoleRegistration>>;
}

/// Reads and changes Discord guild member role membership.
#[allow(async_fn_in_trait)]
pub trait MemberRoleGateway {
    /// Returns whether the bot can assign and remove `role_id` in `guild_id`.
    ///
    /// # Errors
    ///
    /// Returns an error when Discord cannot retrieve the bot's guild membership or role hierarchy.
    async fn can_manage_role(&self, guild_id: &str, role_id: &str) -> Result<bool>;

    /// Returns the Discord role IDs currently assigned to a member.
    ///
    /// # Errors
    ///
    /// Returns an error when Discord cannot retrieve the member.
    async fn fetch_member_roles(&self, guild_id: &str, user_id: &str) -> Result<Vec<String>>;

    /// Applies a role membership change for a guild member.
    ///
    /// # Errors
    ///
    /// Returns an error when Discord rejects the requested membership change.
    async fn modify_user_role(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str,
        action: RoleMembershipAction,
    ) -> Result<()>;
}
