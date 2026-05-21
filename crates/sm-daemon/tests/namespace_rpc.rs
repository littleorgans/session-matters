mod common;

use common::{LOCAL_UID, TestDaemon, local_context};
use sm_core::{
    Namespace, NamespaceCreateRequest, NamespaceGetRequest, NamespaceListRequest, RpcRequest,
    RpcResponse, RuntimeKind, SpawnRequest,
};

#[tokio::test]
async fn namespace_create_get_and_list_are_idempotent() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();

    let created = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::NamespaceCreate {
                request: NamespaceCreateRequest {
                    slug: "alpha".to_string(),
                },
            },
        )
        .await;
    let RpcResponse::NamespaceCreated { response } = created.response else {
        panic!("expected namespace create response");
    };
    assert!(response.created);
    assert_eq!(response.namespace.namespace.as_str(), "alpha");

    let recreated = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::NamespaceCreate {
                request: NamespaceCreateRequest {
                    slug: "alpha".to_string(),
                },
            },
        )
        .await;
    let RpcResponse::NamespaceCreated { response } = recreated.response else {
        panic!("expected namespace create response");
    };
    assert!(!response.created);
    assert_eq!(response.namespace.namespace.as_str(), "alpha");

    let got = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::NamespaceGet {
                request: NamespaceGetRequest {
                    slug: "alpha".to_string(),
                },
            },
        )
        .await;
    let RpcResponse::NamespaceGot { response } = got.response else {
        panic!("expected namespace get response");
    };
    assert_eq!(
        response
            .namespace
            .expect("namespace exists")
            .namespace
            .as_str(),
        "alpha"
    );

    let listed = daemon
        .state
        .handle(
            context,
            RpcRequest::NamespaceList {
                request: NamespaceListRequest::default(),
            },
        )
        .await;
    let RpcResponse::NamespacesListed { response } = listed.response else {
        panic!("expected namespace list response");
    };
    assert_eq!(
        response
            .namespaces
            .iter()
            .map(|record| record.namespace.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "default"]
    );
}

#[tokio::test]
async fn namespace_create_rejects_reserved_and_bad_slugs() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();

    for slug in ["default", "Alpha"] {
        let created = daemon
            .state
            .handle(
                context.clone(),
                RpcRequest::NamespaceCreate {
                    request: NamespaceCreateRequest {
                        slug: slug.to_string(),
                    },
                },
            )
            .await;
        let RpcResponse::Error { message } = created.response else {
            panic!("expected namespace create error");
        };
        assert!(message.contains("namespace"));
    }
}

#[tokio::test]
async fn spawn_uses_strict_create_before_spawn_policy() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();

    let missing = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::Spawn {
                request: spawn_request(
                    daemon._dir.path().display().to_string(),
                    Namespace::new("alpha").expect("namespace validates"),
                ),
            },
        )
        .await;
    let RpcResponse::Error { message } = missing.response else {
        panic!("expected spawn error");
    };
    assert!(message.contains("namespace not found: alpha"));

    daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::NamespaceCreate {
                request: NamespaceCreateRequest {
                    slug: "alpha".to_string(),
                },
            },
        )
        .await;

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: spawn_request(
                    daemon._dir.path().display().to_string(),
                    Namespace::new("alpha").expect("namespace validates"),
                ),
            },
        )
        .await;
    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    assert_eq!(response.session.namespace.as_str(), "alpha");
}

fn spawn_request(dir: String, namespace: Namespace) -> SpawnRequest {
    SpawnRequest {
        runtime: RuntimeKind::Claude,
        role: "pm".to_string(),
        workspace: String::new(),
        dir: Some(dir),
        namespace: Some(namespace),
        target: "headless".to_string(),
        agent_config: None,
        env: Vec::new(),
        shell_resume: None,
        labels: Vec::new(),
    }
}
