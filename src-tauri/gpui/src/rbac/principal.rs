//! The user name and groups RBAC subjects are matched against, ported from
//! `src/features/rbac/principal.ts`. Kubernetes matches a ServiceAccount
//! subject by its username, so a User subject named
//! `system:serviceaccount:<namespace>:<name>` grants that ServiceAccount.

use kubecove_lib::models::{RbacAccessReviewIdentity, RbacSubjectSummary};

const SERVICE_ACCOUNT_PREFIX: &str = "system:serviceaccount:";

/// `automaticServiceAccountGroups`.
pub fn automatic_groups(namespace: &str) -> [String; 3] {
    [
        "system:serviceaccounts".into(),
        format!("system:serviceaccounts:{namespace}"),
        "system:authenticated".into(),
    ]
}

/// `serviceaccount.MakeUsername` in Kubernetes.
pub fn service_account_username(namespace: &str, name: &str) -> String {
    format!("{SERVICE_ACCOUNT_PREFIX}{namespace}:{name}")
}

/// `serviceaccount.SplitUsername`: the namespace and name a ServiceAccount
/// username names, or `None` for any other username.
pub fn split_service_account_username(username: &str) -> Option<(&str, &str)> {
    let (namespace, name) = username
        .strip_prefix(SERVICE_ACCOUNT_PREFIX)?
        .split_once(':')?;
    (!namespace.is_empty() && !name.is_empty() && !name.contains(':')).then_some((namespace, name))
}

/// The authenticated user a SubjectAccessReview sends for an identity, with
/// the backend's mapping in `rbac_review.rs`.
pub struct Principal {
    username: Option<String>,
    groups: Vec<String>,
}

impl Principal {
    pub fn of(identity: &RbacAccessReviewIdentity) -> Self {
        match identity {
            RbacAccessReviewIdentity::ServiceAccount { name, namespace } => Self {
                username: Some(service_account_username(namespace, name)),
                groups: automatic_groups(namespace).into(),
            },
            RbacAccessReviewIdentity::User { username, groups } => Self {
                username: Some(username.clone()),
                groups: groups.clone(),
            },
            RbacAccessReviewIdentity::Group { group } => Self {
                username: None,
                groups: vec![group.clone()],
            },
        }
    }

    pub fn service_account(namespace: &str, name: &str) -> Self {
        Self::of(&RbacAccessReviewIdentity::ServiceAccount {
            name: name.to_owned(),
            namespace: namespace.to_owned(),
        })
    }

    /// `appliesToUser` in the Kubernetes RBAC authorizer.
    pub fn matches(&self, subject: &RbacSubjectSummary) -> bool {
        match subject.kind.as_str() {
            "User" => self.username.as_deref() == Some(subject.name.as_str()),
            "ServiceAccount" => self
                .username
                .as_deref()
                .and_then(split_service_account_username)
                .is_some_and(|(namespace, name)| {
                    subject.namespace.as_deref() == Some(namespace) && subject.name == name
                }),
            "Group" => self.groups.contains(&subject.name),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Failure modes: a ServiceAccount username in a User subject is ignored;
    //! a malformed or prefix-only username matches an account; a username
    //! for another namespace or name matches; a ServiceAccount subject with
    //! no namespace matches; a User identity gains or loses groups the
    //! verifier does not send; a Group identity matches a User subject.
    use super::*;

    fn subject(kind: &str, name: &str, namespace: Option<&str>) -> RbacSubjectSummary {
        RbacSubjectSummary {
            kind: kind.into(),
            name: name.into(),
            namespace: namespace.map(Into::into),
        }
    }

    #[test]
    fn splits_only_well_formed_service_account_usernames() {
        assert_eq!(
            split_service_account_username("system:serviceaccount:payments:api"),
            Some(("payments", "api"))
        );
        for username in [
            "system:serviceaccount:payments",
            "system:serviceaccount::api",
            "system:serviceaccount:payments:",
            "system:serviceaccount:payments:api:extra",
            "system:serviceaccounts:payments:api",
            "alice",
        ] {
            assert_eq!(split_service_account_username(username), None, "{username}");
        }
    }

    #[test]
    fn service_account_matches_its_username_and_groups_only() {
        let account = Principal::service_account("payments", "api");
        assert!(account.matches(&subject("User", "system:serviceaccount:payments:api", None)));
        assert!(account.matches(&subject("ServiceAccount", "api", Some("payments"))));
        assert!(account.matches(&subject("Group", "system:serviceaccounts:payments", None)));
        assert!(!account.matches(&subject("User", "system:serviceaccount:other:api", None)));
        assert!(!account.matches(&subject("User", "system:serviceaccount:payments:web", None)));
        assert!(!account.matches(&subject("ServiceAccount", "api", None)));
        assert!(!account.matches(&subject("Group", "system:serviceaccounts:other", None)));
    }

    #[test]
    fn user_named_as_service_account_matches_that_account_subject() {
        let user = Principal::of(&RbacAccessReviewIdentity::User {
            username: "system:serviceaccount:payments:api".into(),
            groups: vec![],
        });
        assert!(user.matches(&subject("ServiceAccount", "api", Some("payments"))));
        assert!(!user.matches(&subject("Group", "system:authenticated", None)));
    }

    #[test]
    fn group_identity_matches_only_its_group() {
        let group = Principal::of(&RbacAccessReviewIdentity::Group {
            group: "ops".into(),
        });
        assert!(group.matches(&subject("Group", "ops", None)));
        assert!(!group.matches(&subject("User", "ops", None)));
        assert!(!group.matches(&subject("User", "", None)));
    }
}
