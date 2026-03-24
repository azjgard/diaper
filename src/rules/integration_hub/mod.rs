// Re-export shared infrastructure so rule files can use `super::` unchanged.
#[allow(unused_imports)]
pub use super::{AstCache, Rule, RuleViolation, is_excluded_file, parse_js};

pub mod async_await;
pub mod distinct_array;
pub mod file_too_long;
pub mod missing_test;
pub mod nested_ternary;
pub mod new_date;
pub mod non_default_export;
pub mod reduce_param_name;
pub mod short_iter_param;
pub mod unsorted_string_array;

/// Returns all rules for the integration-hub repo.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(async_await::AsyncAwait),
        Box::new(distinct_array::DistinctArray),
        Box::new(file_too_long::FileTooLong),
        Box::new(missing_test::MissingTest),
        Box::new(nested_ternary::NestedTernary),
        Box::new(new_date::NewDate),
        Box::new(non_default_export::NonDefaultExport),
        Box::new(reduce_param_name::ReduceParamName),
        Box::new(short_iter_param::ShortIterParam),
        Box::new(unsorted_string_array::UnsortedStringArray),
    ]
}
