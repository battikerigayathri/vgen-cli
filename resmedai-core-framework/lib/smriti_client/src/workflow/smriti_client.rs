use async_trait::async_trait;

use crate::workflow::client::WorkflowDefinitionClient;
use crate::workflow::instance_client::WorkflowInstanceClient;

/// Unified Smriti SDK trait — definition reads + workflow instance CRUD.
#[async_trait]
pub trait SmritiClient: WorkflowDefinitionClient + WorkflowInstanceClient {}

impl<T> SmritiClient for T where T: WorkflowDefinitionClient + WorkflowInstanceClient {}
