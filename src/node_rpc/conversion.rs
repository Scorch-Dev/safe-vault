// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

use crate::node_rpc::{Error, JSONRPC_NODERPC_ERROR, Query, QueryContainer, Result, ResponseContainer};
use log::error;
use qjsonrpc::{JsonRpcRequest, JsonRpcResponse};
use serde_json::json;
use std::convert::{From, TryFrom};

/*=========Conversions for ResponseContainer=========*/

impl From<ResponseContainer> for JsonRpcResponse {

    /// Convert a response container to a jsonrpc response
    fn from(container: ResponseContainer) -> Self {
        match container.response() {
            Ok(resp) => Self::result(json!(resp), container.id()),
            Err(_) => { 
                Self::error(
                    "TODO: insert error msg".to_string(), 
                    JSONRPC_NODERPC_ERROR, 
                    Some(container.id())
                )
            },
        }
    }
}

/*=========Conversions for QueryContainer=========*/

// Enumerate valid rpc command strings
const METHOD_PING: &str= "ping";
const METHOD_SHUTDOWN: &str = "shutdown";

impl TryFrom<JsonRpcRequest> for QueryContainer {

    type Error = Error;

    /// Convert JsonRpcRequest to a query container
    fn try_from(request: JsonRpcRequest) -> Result<Self> {
        match request.method.as_str() {
            METHOD_PING => Ok(QueryContainer::new(request.id, Query::Ping)),
            METHOD_SHUTDOWN => Ok(QueryContainer::new(request.id, Query::Shutdown)),
            other => {
                let msg = format!(
                    "Method '{}' not supported or unknown by the node manager interface",
                    other
                );
                error!("{}", msg);
                Err(Error::General(msg))
            }
        }
    }

}