// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

use crate::Result;
use serde::{Serialize, Deserialize};

/// Succesful responses to be fed back from the node
/// to the rpc service and eventually are sent out to the client
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Response {

    /// Acknowledge a shutdown
    AckShutdown,

    /// Acknowledge a ping
    AckPing,
}

/// Query response payloads to be shunted on the pipeline from the node
/// to the node manager after the command has been processed
#[derive(Debug)]
pub struct ResponseContainer {

    /// identifies the source of the command
    id: u32,

    /// The result of some query using the nodes' Result type
    response: Result<Response>,
}

impl ResponseContainer {

    /// ctor
    pub fn new(id: u32, response: Result<Response>) -> Self {
        Self {
            id,
            response
        }
    }
    
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn response<'a>(&'a self) -> &'a Result<Response> {
        &self.response
    }
}