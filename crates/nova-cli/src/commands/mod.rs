pub mod runtime;
pub mod config_cmd;
pub mod auth;
pub mod queue;
pub mod scheduler;
pub mod search;
pub mod blob;
pub mod sql;
pub mod db;
pub mod cache;

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
