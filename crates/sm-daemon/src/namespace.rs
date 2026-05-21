use anyhow::{Context, Result};
use chrono::Utc;
use sm_core::{
    Namespace, NamespaceCreateRequest, NamespaceCreateResponse, NamespaceGetRequest,
    NamespaceGetResponse, NamespaceListRequest, NamespaceListResponse, NamespaceRecord,
    RpcResponse,
};
use sm_store::sqlite::NamespaceRecord as StoreNamespaceRecord;

use crate::handler::DaemonState;

impl DaemonState {
    pub(crate) async fn create_namespace(
        &self,
        request: NamespaceCreateRequest,
    ) -> Result<RpcResponse> {
        let namespace = Namespace::for_create(request.slug)?;
        let record = {
            let store = self.store.lock().expect("store lock poisoned");
            let created = if store
                .namespace_exists(&namespace)
                .context("failed to check namespace")?
            {
                false
            } else {
                store
                    .create_namespace(&namespace, Utc::now())
                    .context("failed to create namespace")?;
                true
            };
            let record = store
                .list_namespaces()
                .context("failed to list namespaces")?
                .into_iter()
                .find(|record| record.namespace == namespace)
                .expect("created namespace is listed");
            (record, created)
        };

        Ok(RpcResponse::NamespaceCreated {
            response: NamespaceCreateResponse {
                namespace: namespace_record(record.0),
                created: record.1,
            },
        })
    }

    pub(crate) async fn list_namespaces(
        &self,
        _request: NamespaceListRequest,
    ) -> Result<RpcResponse> {
        let namespaces = self
            .store
            .lock()
            .expect("store lock poisoned")
            .list_namespaces()
            .context("failed to list namespaces")?
            .into_iter()
            .map(namespace_record)
            .collect();

        Ok(RpcResponse::NamespacesListed {
            response: NamespaceListResponse { namespaces },
        })
    }

    pub(crate) async fn get_namespace(&self, request: NamespaceGetRequest) -> Result<RpcResponse> {
        let namespace = Namespace::new(request.slug)?;
        let namespace = self
            .store
            .lock()
            .expect("store lock poisoned")
            .list_namespaces()
            .context("failed to list namespaces")?
            .into_iter()
            .find(|record| record.namespace == namespace)
            .map(namespace_record);

        Ok(RpcResponse::NamespaceGot {
            response: NamespaceGetResponse { namespace },
        })
    }
}

fn namespace_record(record: StoreNamespaceRecord) -> NamespaceRecord {
    NamespaceRecord {
        namespace: record.namespace,
        created_at: record.created_at,
    }
}
