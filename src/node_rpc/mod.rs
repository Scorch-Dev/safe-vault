// copyright 2021 maidsafe.net limited.
//
// this safe network software is licensed to you under the general public license (gpl), version 3.
// unless required by applicable law or agreed to in writing, the safe network software distributed
// under the gpl licence is distributed on an "as is" basis, without warranties or conditions of any
// kind, either express or implied. please review the licences for the specific language governing
// permissions and limitations relating to use of the safe network software.

mod conversion;
mod error;
mod query;
mod response;
// mod stream;

pub use query::{Query, QueryContainer};
pub use response::{Response, ResponseContainer};
pub use error::{Error, Result, JSONRPC_NODERPC_ERROR};

use log::info;
use qjsonrpc::{Endpoint, IncomingJsonRpcRequest, JsonRpcResponse, JsonRpcResponseStream};
use std::{collections::HashMap, convert::TryFrom, path::{Path, PathBuf}, sync::Arc};
use tokio::sync::{mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver}, Mutex};
use url::Url;

/// A JSON RPC over quic interface.
/// Jobs include :JsonRpcResponseStream
///     - receiving `Query` JSON RPCs over quick from the client
///     - sending `QueryContainer`s to the server process containg a wrapped `Query`
///     - receive `ResponseContainer`s back from the server process containing wrapped `Response`s
///     - Sending received `Response`s back to the client using JSON RPC over quic
pub struct NodeManager {

    /// stream to forward Queries to server
    query_tx: UnboundedSender<QueryContainer>,

    /// stream to receive responses from server
    response_rx: UnboundedReceiver<ResponseContainer>,

    /// Maps request id to the response stream
    open_streams: Arc<Mutex<HashMap<u32, JsonRpcResponseStream>>>,
}

impl NodeManager{ 

    /// private ctor
    fn new(
        query_tx: UnboundedSender<QueryContainer>,
        response_rx: UnboundedReceiver<ResponseContainer>,
    ) -> Self {
        let open_streams = Arc::new(Mutex::new(HashMap::new()));
        Self {
            query_tx,
            response_rx,
            open_streams,
        }
    }

    /// use this to insatnce a node manager. Creates the manager
    /// and the proper channels for sending and receiving the containers.
    /// The returned query_rx is fed by the node manager and the returned
    /// response_tx should be fed with responses to queries that come from query_rx.
    /// 
    /// NOTE: If a query, is received by query_rx, it must be serviced 
    /// by pushing a corresponding response into the response_tx (don't ignore queries).
    /// 
    /// If no response is sent to correspond with a query, we'll also leak a bit of memory.
    /// This is because we cache open response streams when a connection is received and
    /// only release them when the response is sent over it
    pub fn init_rpc() -> (Self, UnboundedSender<ResponseContainer>, UnboundedReceiver<QueryContainer>){
        let (query_tx, query_rx) = unbounded_channel::<QueryContainer>();
        let (response_tx, response_rx) = unbounded_channel::<ResponseContainer>();
        (Self::new(query_tx, response_rx), response_tx, query_rx)
    }

    /// Runs the NodeManager
    /// It spins and services new connections and existing connections. 
    /// For the former, it will deserialize Queries, wrap them, and push
    /// them into the query_tx to be serviced. In the case of the former,
    /// when we see a wrapped response on the response_rx, we unwrap it and
    /// send it back to the client who asked for it.
    pub async fn run<P: AsRef<Path>>(
        &mut self,
        listen_addr_raw: &str,
        cert_base_path: Option<P>,
        idle_timeout: Option<u64>,
    ) -> Result<()> {

        // use the default ~/.safe/node_rpc if no path specified
        let base_path = cert_base_path.map_or_else(
            || match dirs_next::home_dir() {
                Some(mut path) => {
                    path.push(".safe");
                    path.push("node_rpc");
                    Ok(path)
                }
                None => Err(Error::General(
                    "Failed to obtain local project directory where to write certificate from"
                        .to_string(),
                )),
            },
            |path| {
                let mut pathbuf = PathBuf::new();
                pathbuf.push(path);
                Ok(pathbuf)
            },
        )?;

        // parse and bind the socket address
        let listen_socket_addr = Url::parse(listen_addr_raw)
            .map_err(|_| Error::General("Invalid endpoint address".to_string()))?
            .socket_addrs(|| None)
            .map_err(|_| Error::General("Invalid endpoint address".to_string()))?[0];

        let qjsonrpc_endpoint = Endpoint::new(base_path, idle_timeout)
            .map_err(|err| Error::General(format!("Failed to create endpoint: {}", err)))?;

        let mut incoming_conn = qjsonrpc_endpoint
            .bind(&listen_socket_addr)
            .map_err(|err| Error::General(format!("Failed to bind endpoint: {}", err)))?;
        info!("Listening on {}", listen_socket_addr);

        // Service requests until we receive confirmation of a shutdown
        // TODO: make loop infinite, check for shutdown, and drain the channel at the end
        let mut shutting_down = false;
        loop {

            tokio::select!(

                // new qjsonrpc connections
                Some(incoming_req) = incoming_conn.get_next(), if !shutting_down => {
                    let _ = tokio::spawn(
                        Self::handle_connection(
                            self.query_tx.clone(),
                            self.open_streams.clone(),
                            incoming_req
                        )
                    );
                },

                // followup on exisxting connection with a resposne
                Some(resp_container) = self.response_rx.recv() => {

                    // watch for shutdown
                    if let Ok(Response::AckShutdown) = resp_container.response() {
                        shutting_down = true;
                    }

                    let _ = tokio::spawn(
                        Self::handle_response(
                            resp_container,
                            self.open_streams.clone()
                        )
                    );
                },

                // break when the senders are dropped
                else => break,
            );
        }

        Ok(())
    }

    /// Handles a completed response coming in over resp_tx
    /// by unwrapping it and forwarding it back to the client
    async fn handle_response(
        resp_container: ResponseContainer,
        open_streams: Arc<Mutex<HashMap<u32, JsonRpcResponseStream>>>
    ) -> Result<()> {

        println!("node mgr: command response found {:?}", resp_container);
        //retreive the stream
        let mut open_streams_lock = open_streams.lock().await;
        let stream_opt = open_streams_lock.remove(&resp_container.id());
        drop(open_streams_lock);
        assert!(stream_opt.is_some());

        // form and send response
        let mut resp_stream = stream_opt
                              .ok_or(Error::General("Couldn't find response stream!".to_string()))?;
        let resp = JsonRpcResponse::from(resp_container);
        println!("node mgr: responding with {:?}", resp);
        resp_stream.respond(&resp).await?;
        println!("node mgr: responded");
        resp_stream.finish().await.map_err(Error::from)
    }

    /// Handle incoming JSON RPC Requests by decoding the Query,
    /// wrapping it, and sending it over the query_tx
    async fn handle_connection(
        query_tx: UnboundedSender<QueryContainer>,
        open_streams: Arc<Mutex<HashMap<u32, JsonRpcResponseStream>>>,
        mut incoming_req: IncomingJsonRpcRequest,
    ) -> Result<()> {

        // Each stream initiated by the client constitutes a new request.
        println!("node mgr: incoming connection");
        while let Some((jsonrpc_req, mut resp_stream)) = incoming_req.get_next().await {

            println!("node mgr: req received {:?}", jsonrpc_req);

            // TODO: validate authority?

            // Try to make a query container from the request
            let id = jsonrpc_req.id;
            match QueryContainer::try_from(jsonrpc_req) {

                // case: push the command to the pipe and cache the response stream
                Ok(cmd) => { 
                    let mut open_streams_lock = open_streams.lock().await;
                    let val = open_streams_lock.insert(id, resp_stream);
                    drop(open_streams_lock);
                    assert!(val.is_none());

                    println!("node mgr: forwarding command to node {:?}", cmd);
                    query_tx.send(cmd)?;
                },

                // case: Malformed request of some sort, so respond with error
                Err(e) => {
                    let resp = &JsonRpcResponse::error(e.to_string(), JSONRPC_NODERPC_ERROR, Some(id));
                    resp_stream.respond(&resp).await.map_err(|e| Error::General(e.to_string()))?;
                    resp_stream.finish().await?;
                },
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {

    use qjsonrpc::ClientEndpoint;
    use super::*;
    use tempdir::TempDir;
    use serde_json::json;

    /// Sets up a minimal client, a fake server process, and a node manager in the middle.
    /// The client pings the manager, which forwards the ping to the server process,
    /// which responds with an ACK to the manager, which forwards the ACK to the client.
    /// A similar flow is then used to send a shutdown signal which undergoes a similar process,
    /// only the minimal server process initiates a shutdown by closing its receiving channel
    /// and responding to the manager with an `AckShutdown`.
    #[tokio::test]
    async fn node_manager_msg_pipeline_test() -> Result<()> {

        // init shared resources
        let listen = "https://localhost:33001";
        let cert_base_dir = Arc::new(TempDir::new("node_mgr_test")?);

        println!("node mgr: instanced");
        let (mut mgr, resp_tx, mut query_rx) = NodeManager::init_rpc();

        // client task
        let cert_base_dir1 = cert_base_dir.clone();
        let client_task = async move {

            let client = ClientEndpoint::new(cert_base_dir1.path(), Some(10000u64), false)?;
            let mut out_conn = client.bind()?;
            println!("client: bound");

            // try ping
            let mut out_jsonrpc_req = out_conn.connect(listen, None).await?;
            println!("client: connected");
            let ack = out_jsonrpc_req.send::<String>("ping", json!(null)).await?;
            println!("client: ping succeeded");
            assert_eq!(ack, "AckPing");

            // try remote shutdown
            let mut out_jsonrpc_req = out_conn.connect(listen, None).await?;
            println!("client: connected");
            let ack = out_jsonrpc_req.send::<String>("shutdown", json!(null)).await?;
            println!("client: shutdown succeeded");
            assert_eq!(ack, "AckShutdown");

            let res: Result<()> = Ok(());
            res
        };

        //  the manager task
        let cert_base_dir2 = cert_base_dir.clone();
        let mgr_task = async move {
            mgr.run(listen, Some(PathBuf::from(cert_base_dir2.path())), Some(10000u64)).await
        };

        // fake "node" actor task to field one command from node manager
        //let resp_tx_clone = resp_tx.clone();
        let fake_node_task = async move {

            // handle the cmd while some senders are open
            while let Some(query_container) = query_rx.recv().await {

                let resp = match query_container.query() {
                    Query::Ping => Ok(Response::AckPing),
                    Query::Shutdown => {
                        query_rx.close(); 
                        Ok(Response::AckShutdown)
                    },
                };

                let resp_container = ResponseContainer::new(query_container.id(), resp);
                println!("node: command received, sending response {:?}", resp_container.response());
                resp_tx.send(resp_container)?;
            }

            Ok(())
        };

        // join all
        tokio::try_join!(mgr_task, client_task, fake_node_task)
            .and_then(|_| Ok(()))
            .map_err(|e| Error::General(e.to_string()))
    }
}