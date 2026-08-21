pub mod auth;
pub mod blob;
pub mod cache;
pub mod config_cmd;
pub mod db;
pub mod queue;
pub mod runtime;
pub mod scheduler;
pub mod search;
pub mod sql;

#[cfg(test)]
mod tests {
    #[test]
    fn test_modules_are_accessible() {
        // Verify that the modules compile and are accessible
        let _ = super::runtime::RuntimeCommands::Status;
        let _ = super::config_cmd::ConfigCommands::Default;
        let _ = super::auth::AuthCommands::ListUsers;
        let _ = super::queue::QueueCommands::List;
        let _ = super::scheduler::SchedulerCommands::List;
        let _ = super::search::SearchCommands::ListIndexes;
        let _ = super::blob::BlobCommands::List { prefix: None };
        let _ = super::sql::SqlCommands::Schema { table: None };
        let _ = super::db::DbCommands::List;
        let _ = super::cache::CacheCommands::Stats;
    }
}
