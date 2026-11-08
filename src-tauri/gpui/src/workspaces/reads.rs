//! Settle context reads independently. Dropping the returned future drops every read.
use crate::backend::{Read, ScopeResult};
use std::{future::poll_fn, task::Poll};

pub fn resources(reads: Vec<(String, Read<ScopeResult>)>) -> Read<ScopeResult> {
    if reads.len() == 1 {
        return reads.into_iter().next().expect("one read").1;
    }
    Box::pin(async move {
        let mut pending: Vec<_> = reads.into_iter().map(Some).collect();
        let mut results = Vec::new();
        poll_fn(|cx| {
            for slot in &mut pending {
                if let Some((context, read)) = slot
                    && let Poll::Ready(result) = read.as_mut().poll(cx)
                {
                    results.push((context.clone(), result));
                    *slot = None;
                }
            }
            if pending.iter().all(Option::is_none) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
        let mut merged = ScopeResult {
            resources: vec![],
            namespaces: vec![],
            warnings: vec![],
            versions: Default::default(),
            listed: vec![],
        };
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
    })
}
