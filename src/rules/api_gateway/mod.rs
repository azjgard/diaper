// Re-export shared infrastructure so rule files can use `super::` unchanged.
pub use super::{AstCache, Rule, RuleViolation, is_excluded_file, parse_js};

pub mod async_await;
pub mod async_directory_name;
pub mod async_promise_return;
pub mod ctx_destructure;
pub mod distinct_array;
pub mod file_too_long;
pub mod graphql_type_export;
pub mod missing_test;
pub mod mock_models;
pub mod nested_ternary;
pub mod new_date;
pub mod non_default_export;
pub mod non_idempotent_migration;
pub mod pipe_property_init;
pub mod reduce_param_name;
pub mod require_query_attributes;
pub mod sequelize_plain_get;
pub mod short_iter_param;
pub mod sql_table_alias;
pub mod unsorted_string_array;
pub mod upward_relative_import;

/// Returns all rules for the api-gateway repo.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(async_await::AsyncAwait),
        Box::new(async_directory_name::AsyncDirectoryName),
        Box::new(async_promise_return::AsyncPromiseReturn),
        Box::new(ctx_destructure::CtxDestructure),
        Box::new(distinct_array::DistinctArray),
        Box::new(file_too_long::FileTooLong),
        Box::new(graphql_type_export::GraphqlTypeExport),
        Box::new(missing_test::MissingTest),
        Box::new(mock_models::MockModels),
        Box::new(new_date::NewDate),
        Box::new(non_default_export::NonDefaultExport),
        Box::new(non_idempotent_migration::NonIdempotentMigration),
        Box::new(pipe_property_init::PipePropertyInit),
        Box::new(reduce_param_name::ReduceParamName),
        Box::new(require_query_attributes::RequireQueryAttributes),
        Box::new(sequelize_plain_get::SequelizePlainGet),
        Box::new(short_iter_param::ShortIterParam),
        Box::new(nested_ternary::NestedTernary),
        Box::new(sql_table_alias::SqlTableAlias),
        Box::new(unsorted_string_array::UnsortedStringArray),
        Box::new(upward_relative_import::UpwardRelativeImport),
    ]
}
