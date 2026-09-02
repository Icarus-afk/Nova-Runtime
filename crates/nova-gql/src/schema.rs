use async_graphql::*;
use std::sync::Arc;

use crate::context::AppContext;
use crate::mutation::MutationRoot;
use crate::query::QueryRoot;
use crate::subscription::SubscriptionRoot;

pub type NovaSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(app: Arc<AppContext>) -> NovaSchema {
    let config = app.config.clone();
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(app)
        .data(config)
        .limit_depth(16)
        .limit_complexity(1000)
        .finish()
}
