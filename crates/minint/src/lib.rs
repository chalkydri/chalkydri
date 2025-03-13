//!
//! # MiniNT
//!
//! A simple NetworkTables library implemented in Rust
//!
//! NetworkTables is a pub-sub messaging system used for FRC.
//!
//! The entrypoint is [NtConn].
//!

// TODO: this needs some cleanup

#[macro_use]
extern crate log;
extern crate rmp;
extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate tokio_tungstenite;

mod datatype;
mod messages;
mod error;

pub use error::{NtError, Result};

use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::net::IpAddr;
use std::sync::Arc;

use datatype::{Data, DataWrap};
use futures_util::stream::{SplitSink, SplitStream};
use messages::*;

use futures_util::{SinkExt, StreamExt};
use rmp::decode::Bytes;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::{
    sync::{mpsc, Mutex},
    task::AbortHandle,
};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header, HeaderValue},
    Error as TungsteniteError, Message,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// A NetworkTables connection
pub struct NtConn {
    /// Next sequential ID
    next_id: Arc<Mutex<i32>>,

    /// Incoming request receiver event loop abort handle
    incoming_abort: Arc<RwLock<Option<AbortHandle>>>,
    /// Outgoing request sender event loop abort handle
    outgoing_abort: Arc<RwLock<Option<AbortHandle>>>,

    /// Outgoing client-to-server message queue
    c2s_tx: mpsc::UnboundedSender<Message>,
    c2s_rx: Arc<Mutex<mpsc::UnboundedReceiver<Message>>>,

    client_ident: Arc<RwLock<String>>,
    server: Arc<RwLock<String>>,
    sock_rd: Arc<RwLock<Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>>,
    sock_wr: Arc<RwLock<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>>,

    topics: Arc<RwLock<HashMap<i32, String>>>,
    topic_pubuids: Arc<RwLock<HashMap<i32, i32>>>,
    pubuid_topics: Arc<RwLock<HashMap<i32, i32>>>,
    values: Arc<RwLock<HashMap<i32, Data>>>,

    /// Mapping from topic names to topic IDs for topics we've received from server 
    server_topics: Arc<RwLock<HashMap<String, i32>>>,
    
    /// Mapping from topic IDs to topic types
    topic_types: Arc<RwLock<HashMap<i32, String>>>,
    
    subscription_values: Arc<RwLock<HashMap<i32, (u64, Data)>>>,
}
impl NtConn {
    /// Connect to a NetworkTables server
    ///
    /// # Arguments
    ///
    /// * `server` - The IP address of the NetworkTables server.
    /// * `client_ident` - The client identifier to use for this connection.
    ///
    /// # Examples
    ///
    /// ```
    /// use minint::NtConn;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Connect to the NetworkTables server at 10.0.0.2
    ///     let conn = NtConn::new("10.0.0.2", "my_client").await.unwrap();
    ///
    ///     // ...
    /// }
    /// ```
    pub async fn new(
        server: impl Into<IpAddr>,
        client_ident: impl Into<String>,
    ) -> Result<Self> {
        let topics = Arc::new(RwLock::new(HashMap::new()));
        let topic_pubuids = Arc::new(RwLock::new(HashMap::new()));
        let pubuid_topics = Arc::new(RwLock::new(HashMap::new()));
        let values = Arc::new(RwLock::new(HashMap::new()));
        let sock_wr: Arc<
            RwLock<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
        > = Arc::new(RwLock::new(None));
        let sock_rd: Arc<RwLock<Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>> =
            Arc::new(RwLock::new(None));

        // Setup channels for control
        let (c2s_tx, c2s_rx) = mpsc::unbounded_channel::<Message>();

        let server_topics = Arc::new(RwLock::new(HashMap::new()));
        let topic_types = Arc::new(RwLock::new(HashMap::new()));
        let subscription_values = Arc::new(RwLock::new(HashMap::new()));

        let conn = Self {
            next_id: Arc::new(Mutex::const_new(0)),
            c2s_tx,
            c2s_rx: Arc::new(Mutex::new(c2s_rx)),

            topics,
            topic_pubuids,
            pubuid_topics,
            values,

            client_ident: Arc::new(RwLock::new(client_ident.into())),
            server: Arc::new(RwLock::new(server.into().to_string())),
            sock_rd,
            sock_wr,

            incoming_abort: Arc::new(RwLock::new(None)),
            outgoing_abort: Arc::new(RwLock::new(None)),

            server_topics,
            topic_types,
            subscription_values,
        };

        conn.init_background_event_loops().await;
        Ok(conn)
    }

    async fn init_background_event_loops(&self) {
        // Spawn event loop to read and process incoming messages

        let mut incoming_abort = self.incoming_abort.write().await;
        let mut outgoing_abort = self.outgoing_abort.write().await;

        if (*incoming_abort).is_none() {
            let conn = self.clone();
            let topics = self.topics.clone();
            let topic_pubuids = self.topic_pubuids.clone();
            let pubuid_topics = self.pubuid_topics.clone();

            let jh = tokio::spawn(async move {
                loop {
                    if let Some(sock_rd) = conn.sock_rd.write().await.as_mut() {
                        while let Some(buf) = sock_rd.next().await {
                            match buf {
                                Ok(Message::Text(json)) => {
                                    let messages: Vec<ServerMsg> =
                                        serde_json::from_str(&json).unwrap();

                                    for msg in messages {
                                        match msg {
                                            ServerMsg::Announce {
                                                name,
                                                id,
                                                r#type,
                                                pubuid,
                                                ..
                                            } => {
                                                // Store server topic info
                                                conn.server_topics.write().await.insert(name.clone(), id);
                                                conn.topic_types.write().await.insert(id, r#type.clone());
                                                
                                                trace!("inserting to topics");
                                                (*topics.write().await).insert(id, name.clone());

                                                if let Some(pubuid) = pubuid {
                                                    trace!("inserting to pubuid_topics");
                                                    (*pubuid_topics.write().await)
                                                        .insert(pubuid, id);
                                                    trace!("inserting to topic_pubuids");
                                                    (*topic_pubuids.write().await)
                                                        .insert(id, pubuid);

                                                    debug!("{name} ({type}): published successfully with topic id {id}");
                                                } else {
                                                    debug!("{name} ({type}): announced with topic id {id}");
                                                }
                                            }
                                            ServerMsg::Unannounce { name, id } => {
                                                let mut topics = topics.write().await;
                                                let topic_pubuids = topic_pubuids.read().await;
                                                let mut pubuid_topics = pubuid_topics.write().await;

                                                (*topics).remove(&id);
                                                if let Some(pubuid) = (*topic_pubuids).get(&id) {
                                                    (*pubuid_topics).remove(pubuid);
                                                }

                                                drop(pubuid_topics);
                                                drop(topic_pubuids);
                                                drop(topics);

                                                debug!("{name}: unannounced");
                                            }
                                            _ => unimplemented!(),
                                        }
                                    }
                                }
                                Ok(Message::Binary(bin)) => {
                                    match Self::read_bin_frame(bin.to_vec()) {
                                        Ok((topic_id, timestamp, data)) => {
                                            trace!("Received binary frame with topic_id={}, ts={}", topic_id, timestamp);
                                            
                                            // Store the value for both general values and subscription-specific values
                                            conn.values.write().await.insert(topic_id as i32, data.clone());
                                            conn.subscription_values.write().await.insert(topic_id as i32, (timestamp, data));
                                        }
                                        Err(err) => {
                                            error!("Failed to parse binary frame: {}", err);
                                        }
                                    }
                                }
                                Ok(msg) => warn!("unhandled incoming message: {msg:?}"),
                                Err(TungsteniteError::ConnectionClosed) => {
                                    conn.connect().await.unwrap();
                                }
                                Err(err) => error!("error reading incoming message: {err:?}"),
                            }
                        }
                    }
                    tokio::task::yield_now().await;
                }
            });

            *incoming_abort = Some(jh.abort_handle());
        }

        // Spawn event loop to send outgoing messages
        if (*outgoing_abort).is_none() {
            let conn = self.clone();

            let jh = tokio::spawn(async move {
                loop {
                    while let Some(outgoing) = conn.c2s_rx.lock().await.recv().await {
                        trace!("sending {outgoing:?}");

                        if let Some(sock_wr) = conn.sock_wr.write().await.as_mut() {
                            match sock_wr.send(outgoing).await {
                                Ok(()) => {
                                    trace!("sent outgoing message successfully");
                                }
                                Err(TungsteniteError::ConnectionClosed) => {
                                    conn.connect().await.unwrap();
                                }
                                Err(err) => {
                                    error!("error writing outgoing message: {err:?}");
                                }
                            }

                            trace!("sent");
                        }
                    }

                    tokio::task::yield_now().await;
                }
            });

            *outgoing_abort = Some(jh.abort_handle());
        }
    }

    async fn connect(&self) -> Result<()> {
        // Get server and client_ident into something we can work with
        let server = self.server.read().await.clone();
        let client_ident = self.client_ident.read().await.clone();

        // Build the WebSocket URL and turn it into tungstenite's client req type
        let mut req = format!("ws://{server}:5810/nt/{client_ident}").into_client_request()?;

        // Add header as specified in WPILib's spec
        req.headers_mut().append(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("v4.1.networktables.first.wpi.edu"),
        );
        // Connect to the server and split into read and write
        let (sock, _) = tokio_tungstenite::connect_async(req).await?;
        let (sock_wr, sock_rd) = sock.split();

        *self.sock_rd.write().await = Some(sock_rd);
        *self.sock_wr.write().await = Some(sock_wr);

        Ok(())
    }

    async fn next_id(&self) -> i32 {
        let next = &mut *self.next_id.lock().await;
        let curr = (*next).clone();
        *next += 1;

        curr
    }

    /// Publish a topic
    ///
    /// The topic will be unpublished when the [NtTopic] is dropped.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the topic to publish.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of data to be published on the topic. Must implement the `DataType` trait.
    ///
    /// # Examples
    ///
    /// ```
    /// use minint::{NtConn, datatype::DataType};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Connect to the NetworkTables server
    ///     let conn = NtConn::new("10.0.0.2", "my_client").await.unwrap();
    ///
    ///     // Publish a new topic named "my_topic" with data type f64
    ///     let mut topic = conn.publish::<f64>("my_topic").await.unwrap();
    ///
    ///     // ...
    /// }
    /// ```
    pub async fn publish<T: DataWrap>(
        &self,
        name: impl Into<String>,
    ) -> Result<NtTopic<T>> {
        let pubuid = self.next_id().await;
        let name = name.into();

        trace!("publishing {name} with pubuid {pubuid}");

        let buf = serde_json::to_string(&[ClientMsg::Publish {
            pubuid,
            name: name.clone(),
            r#type: T::STRING.to_string(),
            properties: Some(PublishProps {
                persistent: Some(false),
                retained: Some(false),
            }),
        }])?;

        self.c2s_tx.send(Message::Text(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))?;

        debug!(
            "{name} ({data_type}): publishing with pubuid {pubuid}",
            data_type = T::STRING.to_string()
        );

        //let mut published = false;
        //while !published {
        //    published = if (*self.pubuid_topics.read().await).contains_key(&pubuid) {
        //        true
        //    } else {
        //        false
        //    };
        //    trace!("waiting for topic to be published");
        //    tokio::time::sleep(Duration::from_millis(100)).await;
        //}

        Ok(NtTopic {
            conn: &*self,
            pubuid,
            _marker: PhantomData,
        })
    }

    /// Unpublish topic
    ///
    /// This method is typically called when an `NtTopic` is dropped.
    fn unpublish(&self, pubuid: i32) -> Result<()> {
        let buf = serde_json::to_string(&[ClientMsg::Unpublish { pubuid }])?;
        self.c2s_tx.send(Message::Text(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))?;

        Ok(())
    }

    /// Subscribe to a topic
    ///
    /// # Arguments
    ///
    /// * `topic` - The name of the topic to subscribe to.
    ///
    /// # Examples
    ///
    /// ```
    /// use minint::NtConn;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Connect to the NetworkTables server
    ///     let conn = NtConn::new("10.0.0.2", "my_client").await.unwrap();
    ///
    ///     // Subscribe to the topic named "my_topic"
    ///     let subscription = conn.subscribe("my_topic").await.unwrap();
    ///
    ///     // ...
    /// }
    /// ```
    pub async fn subscribe(&self, topic: &str) -> Result<NtSubscription> {
        let subuid = self.next_id().await;

        let buf = serde_json::to_string(&[ClientMsg::Subscribe {
            topics: Vec::from_iter([topic.to_string()]),
            subuid,
            options: BTreeMap::new(),
        }])?;
        self.c2s_tx.send(Message::Text(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))?;

        Ok(NtSubscription {
            conn: &*self,
            subuid,
        })
    }

    /// Unsubscribe from a topic
    ///
    /// This method is typically called when an `NtSubscription` is dropped.
    fn unsubscribe(&self, subuid: i32) -> Result<()> {
        let buf = serde_json::to_string(&[ClientMsg::Unsubscribe { subuid }])?;
        self.c2s_tx.send(Message::Text(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))?;

        Ok(())
    }

    /// Read/parse a binary frame
    ///
    /// This method is used internally to process incoming data values for subscribed topics.
    ///
    /// Returns `(uid, timestamp, data)`
    fn read_bin_frame(buf: Vec<u8>) -> Result<(u64, u64, Data)> {
        let mut bytes = Bytes::new(&buf);
        let len = rmp::decode::read_array_len(&mut bytes)?;

        if len == 4 {
            let uid = rmp::decode::read_u64(&mut bytes)?;
            let ts = rmp::decode::read_u64(&mut bytes)?;
            let data_type = rmp::decode::read_u8(&mut bytes)?;
            let data = Data::from(&mut bytes, data_type)
                .map_err(|_| NtError::MessagePackError("Failed to parse data value".to_string()))?;

            Ok((uid, ts, data))
        } else {
            Err(NtError::BinaryFrameError)
        }
    }

    /// Write a binary frame
    ///
    /// This method is used internally to send data values to the NetworkTables server.
    fn write_bin_frame<T: DataWrap>(
        &self,
        uid: i32,
        ts: u64,
        value: T,
    ) -> Result<()> {
        let mut buf = Vec::new();
        rmp::encode::write_array_len(&mut buf, 4)?;

        rmp::encode::write_i32(&mut buf, uid)?;
        rmp::encode::write_uint(&mut buf, ts)?;
        rmp::encode::write_u8(&mut buf, T::MSGPCK)?;
        T::encode(&mut buf, value).map_err(|_| NtError::MessagePackError(
            "Failed to encode value to MessagePack format.".to_string()
        ))?;

        self.c2s_tx.send(Message::Binary(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))?;

        Ok(())
    }

    /// Shutdown the connection
    ///
    /// This method stops the event loops for sending and receiving messages. All `NtTopic`
    /// instances associated with this connection must be dropped before calling this method.
    pub async fn stop(self) {
        // Attempt to unwrap and use incoming and outgoing abort handles

        if let Some(ah) = self.incoming_abort.read().await.as_ref() {
            ah.abort();
        }
        if let Some(ah) = self.outgoing_abort.read().await.as_ref() {
            ah.abort();
        }
    }
}
impl Clone for NtConn {
    fn clone(&self) -> Self {
        Self {
            next_id: self.next_id.clone(),

            incoming_abort: self.incoming_abort.clone(),
            outgoing_abort: self.outgoing_abort.clone(),

            c2s_rx: self.c2s_rx.clone(),
            c2s_tx: self.c2s_tx.clone(),

            client_ident: self.client_ident.clone(),
            server: self.server.clone(),
            sock_wr: self.sock_wr.clone(),
            sock_rd: self.sock_rd.clone(),

            topics: self.topics.clone(),
            topic_pubuids: self.topic_pubuids.clone(),
            pubuid_topics: self.pubuid_topics.clone(),
            values: self.values.clone(),

            server_topics: self.server_topics.clone(),
            topic_types: self.topic_types.clone(),
            subscription_values: self.subscription_values.clone(),
        }
    }
}

/// A NetworkTables topic
///
/// This structure represents a published topic on the NetworkTables server. It allows you to set
/// the value of the topic. The topic is automatically unpublished when this structure is dropped.
pub struct NtTopic<'nt, T: DataWrap> {
    conn: &'nt NtConn,
    pubuid: i32,
    _marker: PhantomData<T>,
}
impl<T: DataWrap + std::fmt::Debug> NtTopic<'_, T> {
    /// Set the value of the topic.
    ///
    /// # Arguments
    ///
    /// * `val` - The new value to set the topic to.
    ///
    /// # Examples
    ///
    /// ```
    /// use minint::{NtConn, datatype::DataType};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Connect to the NetworkTables server
    ///     let conn = NtConn::new("10.0.0.2", "my_client").await.unwrap();
    ///
    ///     // Publish a new topic
    ///     let mut topic = conn.publish::<f64>("my_topic").await.unwrap();
    ///
    ///     // Set the value of the topic
    ///     topic.set(3.14159).await.unwrap();
    ///
    ///     // ...
    /// }
    /// ```
    pub async fn set(&mut self, val: T) -> Result<()> {
        trace!("getting read lock pubuid_topics");
        if let Some(id) = (*self.conn.pubuid_topics.read().await).get(&self.pubuid) {
            trace!("getting read lock topics");
            if let Some(name) = (*self.conn.topics.read().await).get(&id) {
                debug!(
                    "{name} ({data_type}): set to {val:?}",
                    data_type = T::STRING.to_string()
                );
            }
        }

        trace!("writing binary frame");
        (*self.conn).write_bin_frame(self.pubuid, 0, val)?;

        Ok(())
    }
}
impl<T: DataWrap> Drop for NtTopic<'_, T> {
    fn drop(&mut self) {
        if let Err(e) = self.conn.unpublish(self.pubuid) {
            error!("Failed to unpublish topic: {}", e);
        }
    }
}

/// A NetworkTables subscription
///
/// This structure represents a subscription to a topic on the NetworkTables server. It is
/// automatically unsubscribed when this structure is dropped.
pub struct NtSubscription<'nt> {
    conn: &'nt NtConn,
    subuid: i32,
}
impl NtSubscription<'_> {
    pub async fn get(&self) -> Result<Option<(u64, Data)>> {
        Ok(self.conn.subscription_values
            .read()
            .await
            .get(&self.subuid)
            .cloned())
    }
}
impl Drop for NtSubscription<'_> {
    fn drop(&mut self) {
        self.conn.unsubscribe(self.subuid).unwrap();
    }
}
