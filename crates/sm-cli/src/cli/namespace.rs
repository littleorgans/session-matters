use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use sm_core::{
    NamespaceCreateRequest, NamespaceCreateResponse, NamespaceGetRequest, NamespaceListRequest,
    NamespaceRecord, RpcRequest, RpcResponse, SmEndpoint,
};

use crate::cli::cli_def::{
    CreateArgs, CreateResource, InitArgs, InitResource, NamespaceCreateArgs, NamespaceInitArgs,
};
use crate::cli::namespace_resolver::marker_path;

pub async fn create(args: CreateArgs) -> Result<()> {
    match args.resource {
        CreateResource::Namespace(args) => create_namespace(args).await,
    }
}

pub async fn init(args: InitArgs) -> Result<()> {
    match args.resource {
        InitResource::Namespace(args) => init_namespace(args).await,
    }
}

pub async fn get(slug: Option<String>, json: bool) -> Result<()> {
    match slug {
        Some(slug) => get_one(slug, json).await,
        None => list(json).await,
    }
}

async fn list(json: bool) -> Result<()> {
    let response = send(&RpcRequest::NamespaceList {
        request: NamespaceListRequest::default(),
    })
    .await?;

    match response {
        RpcResponse::NamespacesListed { response } if json => {
            println!("{}", serde_json::to_string_pretty(&response.namespaces)?);
            Ok(())
        }
        RpcResponse::NamespacesListed { response } => {
            print_namespaces(&response.namespaces);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}

async fn get_one(slug: String, json: bool) -> Result<()> {
    let response = send(&RpcRequest::NamespaceGet {
        request: NamespaceGetRequest { slug: slug.clone() },
    })
    .await?;

    match response {
        RpcResponse::NamespaceGot { response } if json => {
            let namespace = response
                .namespace
                .ok_or_else(|| anyhow::anyhow!("unknown namespace: {slug}"))?;
            println!("{}", serde_json::to_string_pretty(&namespace)?);
            Ok(())
        }
        RpcResponse::NamespaceGot { response } => {
            let namespace = response
                .namespace
                .ok_or_else(|| anyhow::anyhow!("unknown namespace: {slug}"))?;
            print_namespaces(&[namespace]);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}

async fn create_namespace(args: NamespaceCreateArgs) -> Result<()> {
    let response = request_create(args.slug).await?;
    print_create_response(&response);
    Ok(())
}

async fn init_namespace(args: NamespaceInitArgs) -> Result<()> {
    let dir = args.dir.unwrap_or(std::env::current_dir()?);
    let dir = std::fs::canonicalize(&dir).with_context(|| {
        format!(
            "failed to canonicalize namespace directory {}",
            dir.display()
        )
    })?;
    let marker = marker_path(&dir);
    ensure_marker_available(&marker, &args.slug)?;

    let response = request_create(args.slug.clone()).await?;
    write_marker(&marker, &args.slug)?;
    print_create_response(&response);
    println!("wrote namespace marker: {}", marker.display());
    Ok(())
}

async fn request_create(slug: String) -> Result<NamespaceCreateResponse> {
    let response = send(&RpcRequest::NamespaceCreate {
        request: NamespaceCreateRequest { slug },
    })
    .await?;

    match response {
        RpcResponse::NamespaceCreated { response } => Ok(response),
        RpcResponse::Error { message } => bail!(message),
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}

async fn send(request: &RpcRequest) -> Result<RpcResponse> {
    let endpoint = SmEndpoint::from_env()?;
    sm_daemon::send_request(&endpoint, request).await
}

fn ensure_marker_available(marker: &Path, slug: &str) -> Result<()> {
    if !marker.exists() {
        return Ok(());
    }
    let existing = std::fs::read_to_string(marker)
        .with_context(|| format!("failed to read namespace marker {}", marker.display()))?;
    if existing.trim() == slug {
        return Ok(());
    }
    bail!(
        "namespace marker already exists with different content: {}",
        marker.display()
    );
}

fn write_marker(marker: &Path, slug: &str) -> Result<()> {
    let parent = marker
        .parent()
        .ok_or_else(|| anyhow::anyhow!("namespace marker has no parent: {}", marker.display()))?;
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create namespace marker directory {}",
            parent.display()
        )
    })?;
    let tmp = marker_tmp_path(marker);
    std::fs::write(&tmp, format!("{slug}\n"))
        .with_context(|| format!("failed to write namespace marker {}", tmp.display()))?;
    std::fs::rename(&tmp, marker)
        .with_context(|| format!("failed to install namespace marker {}", marker.display()))?;
    Ok(())
}

fn marker_tmp_path(marker: &Path) -> PathBuf {
    marker.with_extension(format!("tmp-{}", std::process::id()))
}

fn print_create_response(response: &NamespaceCreateResponse) {
    if response.created {
        println!("created namespace: {}", response.namespace.namespace);
    } else {
        println!("namespace already exists: {}", response.namespace.namespace);
    }
}

fn print_namespaces(namespaces: &[NamespaceRecord]) {
    println!("NAMESPACE CREATED_AT");
    for record in namespaces {
        println!("{} {}", record.namespace, record.created_at.to_rfc3339());
    }
}
