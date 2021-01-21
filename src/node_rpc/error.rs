// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

//use crate::node_rpc::stream;
use crate::node_rpc::{QueryContainer, ResponseContainer};
use thiserror::Error;
use std::io;
use tokio::sync::mpsc;

// Error code in JSON-RPC response when failed to process a request
// TODO: have different error codes for each error case
pub const JSONRPC_NODERPC_ERROR: isize = -1;

/// NodeManager Error Variant 
#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum Error {

    /// Io Error
    #[error("Io error: {0}")]
    Io(#[from] io::Error),

    /// jsonrpc error
    /// TODO: implement std::error::Error in qjsonrpc::Error type in sn_api
    #[error("QJsonRpc error::{0}")]
    QJsonRpc(qjsonrpc::Error),

    /// Send Error for Query Containers
    #[error("SendError for QueryContainer to channel (was the receiver dropped?): {0}")]
    QueryContainerSend(#[from] mpsc::error::SendError<QueryContainer>),
    
    /// Send Error for Response Containers
    #[error("SendError for ResponseContainer to channel (was the receiver dropped?): {0}")]
    ResponseContainerSend(#[from] mpsc::error::SendError<ResponseContainer>),

    /// "misc" errors caused by uncommon events or to map another Result
    /// E.g. to wrap a tokio task which returns Result<Result<()>, JoinError>
    /// to a Result<Result<()>, Error::General>
    #[error("NodeManager General Error: {0}")]
    General(String),
}

impl From<qjsonrpc::Error> for Error {
    fn from(err: qjsonrpc::Error) -> Self {
        Error::QJsonRpc(err)
    }
}

pub type Result<T, E=Error> = std::result::Result<T,E>;