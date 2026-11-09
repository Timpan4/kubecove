//! Settle context reads independently. Dropping the returned future drops every read.
use crate::backend::{Read, ScopeResult};
use futures_util::future::join_all;
use kubecove_lib::models::AppError;
use std::{future::Future, pin::Pin};

/// Each context's result, in plan order.
pub type ContextResults<T> = Vec<(String, Result<T, AppError>)>;

/// Every context's read, settled.
pub type Settled<T> = Pin<Box<dyn Future<Output = ContextResults<T>> + Send>>;

/// Runs every context's read at once and returns the results in plan order,
/// not completion order. One failed read does not cancel the others.
pub fn settle<T: Send + 'static>(reads: Vec<(String, Read<T>)>) -> Settled<T> {
    let (contexts, reads): (Vec<_>, Vec<_>) = reads.into_iter().unzip();
    Box::pin(async move { contexts.into_iter().zip(join_all(reads).await).collect() })
}

/// The Overview's read of several contexts as one result: their rows and lists,
/// namespaces once each, and warnings named after their context. It fails only
/// when every context failed. One context's result stays as it is.
pub fn merge(results: ContextResults<ScopeResult>) -> Result<ScopeResult, AppError> {
    if results.len() == 1 {
        return results.into_iter().next().expect("one result").1;
    }
    let mut merged = ScopeResult::default();
    let mut first_error = None;
    let mut succeeded = false;
    for (context, result) in results {
        match result {
            Ok(result) => {
                succeeded = true;
                merged.resources.extend(result.resources);
                merged.listed.extend(result.listed);
                for ns in result.namespaces {
                    if !merged.namespaces.iter().any(|other| other.name == ns.name) {
                        merged.namespaces.push(ns);
                    }
                }
                merged.warnings.extend(
                    result
                        .warnings
                        .into_iter()
                        .map(|warning| format!("{context}: {warning}")),
                );
            }
            Err(error) => {
                merged
                    .warnings
                    .push(format!("{context}: {}", error.message));
                first_error.get_or_insert(error);
            }
        }
    }
    if !succeeded && let Some(mut error) = first_error {
        error.message = merged.warnings.join("\n");
        return Err(error);
    }
    Ok(merged)
}
