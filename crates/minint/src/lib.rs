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

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use datatype::{Data, DataWrap};
use messages::*;

use futures_util::{SinkExt, StreamExt};
use rmp::decode::Bytes;
use tokio::sync::RwLock;
use tokio::{
    sync::{mpsc, Mutex},
    task::AbortHandle,
};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header, HeaderValue},
    Message,
};

/// A NetworkTables connection
pub struct NtConn {
    /// Next sequential ID
    next_id: Arc<Mutex<i32>>,

    /// Incoming request receiver event loop abort handle
    incoming_abort: Option<AbortHandle>,
    /// Outgoing request sender event loop abort handle
    outgoing_abort: Option<AbortHandle>,

    /// Outgoing client-to-server message queue
    c2s: mpsc::UnboundedSender<Message>,

    topics: Arc<RwLock<HashMap<i32, String>>>,
    topic_pubuids: Arc<RwLock<HashMap<i32, i32>>>,
    pubuid_topics: Arc<RwLock<HashMap<i32, i32>>>,
    values: Arc<RwLock<HashMap<i32, Data>>>,
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
    ) -> Result<Self, Box<dyn Error>> {
        let topics = Arc::new(RwLock::new(HashMap::new()));
        let topic_pubuids = Arc::new(RwLock::new(HashMap::new()));
        let pubuid_topics = Arc::new(RwLock::new(HashMap::new()));
        let values = Arc::new(RwLock::new(HashMap::new()));

        // Get server and client_ident into something we can work with
        let server = server.into();
        let client_ident = client_ident.into();

        // Build the WebSocket URL and turn it into tungstenite's client req type
        let mut req = format!("ws://{server}:5810/nt/{client_ident}").into_client_request()?;

        // Add header as specified in WPILib's spec
        req.headers_mut().append(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("networktables.first.wpi.edu"),
        );

        // Setup channels for control
        let (c2s_tx, mut c2s_rx) = mpsc::unbounded_channel::<Message>();

        // Connect to the server and split into read and write
        let (sock, _) = tokio_tungstenite::connect_async(req).await?;
        let (mut sock_wr, mut sock_rd) = sock.split();

        // Spawn event loop to read and process incoming messages

        let incoming_abort = Some({
            let topics = topics.clone();
            let topic_pubuids = topic_pubuids.clone();
            let pubuid_topics = pubuid_topics.clone();

            tokio::spawn(async move {
                loop {
                    while let Some(buf) = sock_rd.next().await {
                        match buf {
                            Ok(Message::Text(json)) => {
                                let messages: Vec<ServerMsg> = serde_json::from_str(&json).unwrap();

                                for msg in messages {
                                    match msg {
                                        ServerMsg::Announce { name, id, r#type, pubuid, .. } => {
                                            (*topics.write().await).insert(id, name.clone());

                                            if let Some(pubuid) = pubuid {
                                                let mut pubuid_topics = pubuid_topics.write().await;
                                                let mut topic_pubuids = topic_pubuids.write().await;

                                                (*pubuid_topics).insert(pubuid, id);
                                                (*topic_pubuids).insert(id, pubuid);

                                                drop(pubuid_topics);
                                                drop(topic_pubuids);

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
                                        _ => unimplemented!()
                                    }
                                }
                            }
                            Ok(Message::Binary(bin)) => {
                                Self::read_bin_frame(bin).unwrap();
                            },
                            Ok(msg) => warn!("unhandled incoming message: {msg:?}"),
                            Err(err) => error!("error reading incoming message: {err:?}"),
                        }
                    }
                    tokio::task::yield_now().await;
                }
            })
            .abort_handle()
        });

        // Spawn event loop to send outgoing messages
        let outgoing_abort = Some(
            tokio::spawn(async move {
                loop {
                    while let Some(outgoing) = c2s_rx.recv().await {
                        sock_wr.send(outgoing).await.unwrap();
                    }
                    tokio::task::yield_now().await;
                }
            })
            .abort_handle(),
        );

        Ok(Self {
            next_id: Arc::new(Mutex::const_new(0)),
            c2s: c2s_tx,

            topics,
            topic_pubuids,
            pubuid_topics,
            values,

            incoming_abort,
            outgoing_abort,
        })
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
    ) -> Result<NtTopic<T>, Box<dyn Error>> {
        let pubuid = self.next_id().await;
        let name = name.into();

        let buf = serde_json::to_string(&[ClientMsg::Publish {
            pubuid,
            name: name.clone(),
            r#type: T::STRING.to_string(),
            properties: Some(PublishProps {
                persistent: Some(true),
                retained: Some(true),
            }),
        }])?;

        self.c2s.send(Message::Text(buf)).unwrap();

        debug!(
            "{name} ({data_type}): publishing with pubuid {pubuid}",
            data_type = T::STRING.to_string()
        );

        while !(*self.pubuid_topics.read().await).contains_key(&pubuid) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(NtTopic {
            conn: &*self,
            pubuid,
            _marker: PhantomData,
        })
    }

    /// Unpublish topic
    ///
    /// This method is typically called when an `NtTopic` is dropped.
    fn unpublish(&self, pubuid: i32) -> Result<(), Box<dyn Error>> {
        let buf = serde_json::to_string(&[ClientMsg::Unpublish { pubuid }])?;
        self.c2s.send(Message::Text(buf))?;

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
    pub async fn subscribe(&self, topic: &str) -> Result<NtSubscription, Box<dyn Error>> {
        let subuid = self.next_id().await;

        let buf = serde_json::to_string(&[ClientMsg::Subscribe {
            topics: Vec::from_iter([topic.to_string()]),
            subuid,
            options: BTreeMap::new(),
        }])?;
        self.c2s.send(Message::Text(buf))?;

        Ok(NtSubscription {
            conn: &*self,
            subuid,
        })
    }

    /// Unsubscribe from topic(s)
    ///
    /// This method is typically called when an `NtSubscription` is dropped.
    fn unsubscribe(&self, subuid: i32) -> Result<(), Box<dyn Error>> {
        let buf = serde_json::to_string(&[ClientMsg::Unsubscribe { subuid }])?;
        self.c2s.send(Message::Text(buf))?;

        Ok(())
    }

    /// Read/parse a binary frame
    ///
    /// This method is used internally to process incoming data values for subscribed topics.
    ///
    /// Returns `(uid, timestamp, data)`
    fn read_bin_frame(buf: Vec<u8>) -> Result<(u64, u64, Data), ()> {
        let mut bytes = Bytes::new(&buf);
        let len = rmp::decode::read_array_len(&mut bytes).map_err(|_| ())?;

        if len == 4 {
            let uid = rmp::decode::read_u64(&mut bytes).map_err(|_| ())?;
            let ts = rmp::decode::read_u64(&mut bytes).map_err(|_| ())?;
            let data_type = rmp::decode::read_u8(&mut bytes).map_err(|_| ())?;
            let data = Data::from(&mut bytes, data_type).map_err(|_| ())?;

            Ok((uid, ts, data))
        } else {
            Err(())
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
    ) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();
        rmp::encode::write_array_len(&mut buf, 4)?;

        rmp::encode::write_i32(&mut buf, uid)?;
        rmp::encode::write_uint(&mut buf, ts)?;
        rmp::encode::write_u8(&mut buf, T::MSGPCK)?;
        T::encode(&mut buf, value).map_err(|_| std::io::Error::other("i don't fucking know"))?;

        self.c2s.send(Message::Binary(buf))?;

        Ok(())
    }

    /// Shutdown the connection
    ///
    /// This method stops the event loops for sending and receiving messages. All `NtTopic`
    /// instances associated with this connection must be dropped before calling this method.
    pub fn stop(self) {
        // Attempt to unwrap and use incoming and outgoing abort handles

        if let Some(ah) = self.incoming_abort {
            ah.abort();
        }
        if let Some(ah) = self.outgoing_abort {
            ah.abort();
        }
    }
}
impl Clone for NtConn {
    fn clone(&self) -> Self {
        Self {
            next_id: self.next_id.clone(),
            incoming_abort: None,
            outgoing_abort: None,
            c2s: self.c2s.clone(),
            topics: self.topics.clone(),
            topic_pubuids: self.topic_pubuids.clone(),
            pubuid_topics: self.pubuid_topics.clone(),
            values: self.values.clone(),
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
    pub async fn set(&mut self, val: T) -> Result<(), Box<dyn Error>> {
        if let Some(id) = (*self.conn.pubuid_topics.read().await).get(&self.pubuid) {
            if let Some(name) = (*self.conn.topics.read().await).get(&id) {
                debug!(
                    "{name} ({data_type}): set to {val:?}",
                    data_type = T::STRING.to_string()
                );
            }
        }

        (*self.conn).write_bin_frame(self.pubuid, 0, val)?;

        Ok(())
    }
}
impl<T: DataWrap> Drop for NtTopic<'_, T> {
    fn drop(&mut self) {
        self.conn.unpublish(self.pubuid).unwrap();
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
impl NtSubscription<'_> {}
impl Drop for NtSubscription<'_> {
    fn drop(&mut self) {
        self.conn.unsubscribe(self.subuid).unwrap();
    }
}
