use crate::sql::query::Queries;
use crate::sql::schema::User;

use async_trait::async_trait;
use scouter_error::{SqlError, UtilError};

use sqlx::{Pool, Postgres};
use std::result::Result::Ok;

#[async_trait]
pub trait UserSqlLogic {
    /// Inserts a new user into the database.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `user` - The user to insert
    ///
    /// # Returns
    /// * A result indicating success or failure
    async fn insert_user(pool: &Pool<Postgres>, user: &User) -> Result<(), SqlError> {
        let query = Queries::InsertUser.get_query();

        let group_permissions = serde_json::to_value(&user.group_permissions)
            .map_err(UtilError::traced_serialize_error)?;

        let permissions =
            serde_json::to_value(&user.permissions).map_err(UtilError::traced_serialize_error)?;

        sqlx::query(&query.sql)
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&permissions)
            .bind(&group_permissions)
            .bind(&user.role)
            .bind(user.active)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(())
    }

    /// Retrieves a user from the database by username.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `username` - The username of the user to retrieve
    ///
    /// # Returns
    /// * A result containing the user if found, or None if not found
    async fn get_user(pool: &Pool<Postgres>, username: &str) -> Result<Option<User>, SqlError> {
        let query = Queries::GetUser.get_query();

        let user: Option<User> = sqlx::query_as(&query.sql)
            .bind(username)
            .fetch_optional(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(user)
    }

    /// Updates a user in the database.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool
    /// * `user` - The user to update
    ///
    /// # Returns
    /// * A result indicating success or failure
    async fn update_user(pool: &Pool<Postgres>, user: &User) -> Result<(), SqlError> {
        let query = Queries::UpdateUser.get_query();

        let group_permissions = serde_json::to_value(&user.group_permissions)
            .map_err(UtilError::traced_serialize_error)?;

        let permissions =
            serde_json::to_value(&user.permissions).map_err(UtilError::traced_serialize_error)?;

        sqlx::query(&query.sql)
            .bind(user.active)
            .bind(&user.password_hash)
            .bind(&permissions)
            .bind(&group_permissions)
            .bind(&user.refresh_token)
            .bind(&user.username)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(())
    }

    /// Retrieves all users from the database.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// * A result containing a vector of users
    async fn get_users(pool: &Pool<Postgres>) -> Result<Vec<User>, SqlError> {
        let query = Queries::GetUsers.get_query();

        let users = sqlx::query_as::<_, User>(&query.sql)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(users)
    }

    /// Checks if user is the last admin in the system.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `username` - The username of the user to check
    ///
    /// # Returns
    /// * Boolean indicating if the user is the last admin
    async fn is_last_admin(pool: &Pool<Postgres>, username: &str) -> Result<bool, SqlError> {
        // Count admins in the system
        let query = Queries::LastAdmin.get_query();

        let admins: Vec<String> = sqlx::query_scalar(&query.sql)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        // check if length is 1 and the username is the same
        if admins.len() > 1 {
            return Ok(false);
        }

        // no admins found
        if admins.is_empty() {
            return Ok(false);
        }

        // check if the username is the last admin
        Ok(admins.len() == 1 && admins[0] == username)
    }

    /// Deletes a user from the database.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `username` - The username of the user to delete
    async fn delete_user(pool: &Pool<Postgres>, username: &str) -> Result<(), SqlError> {
        let query = Queries::DeleteUser.get_query();

        sqlx::query(&query.sql)
            .bind(username)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(())
    }
}
