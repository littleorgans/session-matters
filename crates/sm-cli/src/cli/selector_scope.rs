use std::str::FromStr;

use anyhow::Result;
use clap::Args;
use sm_core::{Namespace, NamespaceScope, Selector};

use crate::cli::namespace_resolver::resolve_namespace_dir;

#[derive(Debug, Clone, Default, Args)]
pub struct NamespaceScopeArgs {
    #[arg(long, help = "Namespace slug used to scope selector resolution")]
    pub namespace: Option<Namespace>,
    #[arg(
        short = 'A',
        long = "all-namespaces",
        conflicts_with = "namespace",
        help = "Bypass default namespace scoping"
    )]
    pub all_namespaces: bool,
}

pub fn scoped_selector(raw: Option<&str>, scope: &NamespaceScopeArgs) -> Result<Option<Selector>> {
    let selector = raw.map(Selector::from_str).transpose()?;
    if scope.all_namespaces {
        return Ok(selector);
    }
    let namespace =
        resolve_namespace_dir(std::env::current_dir()?, scope.namespace.clone())?.namespace;
    let scope = if scope.namespace.is_some() {
        NamespaceScope::Explicit
    } else {
        NamespaceScope::Default
    };
    Ok(Some(Selector::scoped_to_namespace(
        selector, namespace, scope,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_namespaces_leaves_selector_unscoped() {
        let scope = NamespaceScopeArgs {
            namespace: Some(Namespace::new("alpha").unwrap()),
            all_namespaces: true,
        };
        let selector = scoped_selector(Some("role:engineer"), &scope)
            .expect("selector scopes")
            .expect("selector exists");
        assert_eq!(
            selector,
            Selector::Role {
                name: "engineer".to_string()
            }
        );
    }
}
