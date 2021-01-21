// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

use serde::{Serialize, Deserialize};

/// Payloads for what command to execute when the node gets to
/// processing the command
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Query {

    /// Ping the node
    Ping,

    /// Shutdown the node
    Shutdown,
}

/// QueryContainers to be sent over the pipe 
/// from the node manager process to the node for processing
#[derive(Debug)]
pub struct QueryContainer{ 

    /// identifies the source of the command
    id: u32,

    /// the command to carry out and its params
    query: Query,
}

impl QueryContainer {

    /// construct a new command
    pub fn new(id: u32, query: Query) -> Self {
        Self {
            id,
            query
        }
    }

    // Getter functions below

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn query<'a>(&'a self) -> &'a Query {
        &self.query
    }
}