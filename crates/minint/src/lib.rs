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
extern crate tracing;
extern crate quanta;
extern crate rmp;
extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate tokio_tungstenite;

mod datatype;
mod error;
mod messages;

pub use error::{NtError, Result};
use tokio::time::{interval, Interval};

use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use datatype::{BsInt, Data, DataWrap};
use futures_util::stream::{SplitSink, SplitStream};
use messages::*;

use futures_util::{SinkExt, StreamExt};
use quanta::{Clock, Instant};
use rmp::decode::Bytes;
use tokio::net::TcpStream;
use tokio::sync::{watch, RwLock};
use tokio::{
    sync::{mpsc, Mutex},
    task::AbortHandle,
};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{header, HeaderValue},
    Error as TungsteniteError, Message,
};
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};

// I wanna keep my face on today

async fn reconnector(
    mut rx: mpsc::Receiver<()>,
    server: String,
    client_ident: String,
    sock_rd: Arc<RwLock<Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>>,
    sock_wr: Arc<RwLock<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>>,
) -> Result<()> {
    // Build the WebSocket URL and turn it into tungstenite's client req type
    let mut req = format!("ws://{server}:5810/nt/{client_ident}").into_client_request()?;

    // Add header as specified in WPILib's spec
    req.headers_mut().append(
        header::SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static("v4.1.networktables.first.wpi.edu"),
    );

    loop {
        rx.recv().await;
        {
            let mut sock_rd = sock_rd.write().await;
            let mut sock_wr = sock_wr.write().await;

            *sock_rd = None;
            if let Some(sock_wr) = sock_wr.as_mut() {
                sock_wr.close().await.unwrap();
            }

            // Repeatedly attempt to connect to the server
            loop {
                match tokio_tungstenite::connect_async(req.clone()).await {
                    Ok((sock, _)) => {
                        let (sock_wr_, sock_rd_) = sock.split();

                        {
                            *sock_rd = Some(sock_rd_);
                            *sock_wr = Some(sock_wr_);
                        }

                        info!("connected");
                        break;
                    }
                    Err(err) => {
                        error!("failed to connect: {err:?}");
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // Clear remaining trash
        while let Some(_) = rx.recv().await {}
    }
}

/// A NetworkTables connection
pub struct NtConn {
    start_time: Instant,
    offset: Arc<RwLock<Duration>>,

    reconnect_tx: mpsc::Sender<()>,

    /// Next sequential ID
    next_id: Arc<Mutex<i32>>,

    /// Outgoing client-to-server message queue
    c2s_tx: mpsc::UnboundedSender<Message>,

    /// Incoming request receiver event loop abort handle
    incoming_abort: Arc<RwLock<Option<AbortHandle>>>,
    /// Outgoing request sender event loop abort handle
    outgoing_abort: Arc<RwLock<Option<AbortHandle>>>,
    task_abort: Arc<RwLock<Option<AbortHandle>>>,

    sock_rd: Arc<RwLock<Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>>,
    sock_wr: Arc<RwLock<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>>,

    server_client: Arc<RwLock<HashMap<i32, i32>>>,
    client_server: Arc<RwLock<HashMap<i32, i32>>>,

    /// Mapping from topic names to topic IDs for topics we've received from server
    server_topics: Arc<RwLock<HashMap<String, (i32, String)>>>,

    values: Arc<RwLock<HashMap<i32, watch::Receiver<(u64, Data)>>>>,
    value_tx: Arc<RwLock<HashMap<i32, watch::Sender<(u64, Data)>>>>,
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
    pub async fn new(server: impl Into<String>, client_ident: impl Into<String>) -> Result<Self> {
        let server = server.into();
        let client_ident = client_ident.into();

        let client_server = Arc::new(RwLock::new(HashMap::new()));
        let server_client = Arc::new(RwLock::new(HashMap::new()));
        let sock_wr: Arc<
            RwLock<Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
        > = Arc::new(RwLock::new(None));
        let sock_rd: Arc<RwLock<Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>> =
            Arc::new(RwLock::new(None));

        // Setup channels for control
        let (c2s_tx, c2s_rx) = mpsc::unbounded_channel::<Message>();

        let (reconnect_tx, reconnect_rx) = mpsc::channel::<()>(1);
        let sock_rd_ = sock_rd.clone();
        let sock_wr_ = sock_wr.clone();
        tokio::spawn(async move {
            reconnector(reconnect_rx, server, client_ident, sock_rd_, sock_wr_)
                .await
                .unwrap();
        });

        let server_topics = Arc::new(RwLock::new(HashMap::new()));
        let values = Arc::new(RwLock::new(HashMap::new()));
        let value_tx = Arc::new(RwLock::new(HashMap::new()));

        let start_time = Instant::now();

        let conn = Self {
            start_time,
            offset: Arc::new(RwLock::new(Duration::ZERO)),

            reconnect_tx,
            next_id: Arc::new(Mutex::const_new(0)),

            client_server,
            server_client,

            c2s_tx,

            sock_rd,
            sock_wr,

            incoming_abort: Arc::new(RwLock::new(None)),
            outgoing_abort: Arc::new(RwLock::new(None)),
            task_abort: Arc::new(RwLock::new(None)),

            server_topics,
            values,
            value_tx,
        };

        trace!("initializing background event loop...");
        conn.init_background_event_loops(c2s_rx).await;
        trace!("initialized background event loop...");

        conn.reconnect_tx.send(()).await.unwrap();

        Ok(conn)
    }

    /// Initialize the background event loops
    async fn init_background_event_loops(&self, mut c2s_rx: mpsc::UnboundedReceiver<Message>) {
        // Spawn event loop to read and process incoming messages

        let mut incoming_abort = self.incoming_abort.write().await;
        let mut outgoing_abort = self.outgoing_abort.write().await;
        let mut task_abort = self.task_abort.write().await;

        if (*incoming_abort).is_none() {
            let conn = self.clone();

            let jh = tokio::spawn(async move {
                loop {
                    if let Some(sock_rd) = conn.sock_rd.write().await.as_mut() {
                        while let Some(msg) = sock_rd.next().await {
                            match conn.handle_incoming_msg(msg).await {
                                Err(NtError::NeedReconnect) => {
                                    conn.reconnect_tx.send(()).await;
                                    break;
                                }
                                _ => {}
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
                    while let Some(outgoing) = c2s_rx.recv().await {
                        trace!("sending {outgoing:?}");

                        if let Some(sock_wr) = conn.sock_wr.write().await.as_mut() {
                            match sock_wr.send(outgoing).await {
                                Ok(()) => {
                                    trace!("sent outgoing message successfully");
                                }
                                Err(TungsteniteError::ConnectionClosed)
                                | Err(TungsteniteError::Io(_))
                                | Err(TungsteniteError::AlreadyClosed)
                                | Err(TungsteniteError::Protocol(_)) => {
                                    conn.reconnect_tx.send(()).await;
                                    break;
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

        if (*task_abort).is_none() {
            let conn = self.clone();

            let jh = tokio::spawn(async move {
                let mut ping_interval = interval(Duration::from_millis(500));
                let mut time_correct_interval = interval(Duration::from_secs(3));

                loop {
                    tokio::select! {
                        _ = ping_interval.tick() => conn.ping().await.unwrap(),
                        _ = time_correct_interval.tick() => conn.time_correct().await.unwrap(),
                    }

                    //    if let Some(sock_wr) = conn.sock_wr.write().await.as_mut() {
                    //        match sock_wr.send(outgoing).await {
                    //            Ok(()) => {
                    //                trace!("sent outgoing message successfully");
                    //            }
                    //            Err(TungsteniteError::ConnectionClosed)
                    //            | Err(TungsteniteError::Io(_))
                    //            | Err(TungsteniteError::AlreadyClosed)
                    //            | Err(TungsteniteError::Protocol(_)) => {
                    //                conn.reconnect_tx.send(()).await;
                    //                break;
                    //            }
                    //            Err(err) => {
                    //                error!("error writing outgoing message: {err:?}");
                    //            }
                    //        }

                    //        trace!("sent");
                    //    }
                    //}

                    tokio::task::yield_now().await;
                }
            });

            *task_abort = Some(jh.abort_handle());
        }
    }

    async fn ping(&self) -> Result<()> {
        self.c2s_tx
            .send(Message::Ping(tungstenite::Bytes::from_static(
                b"sigma sigma boy",
            )))
            .unwrap();

        Ok(())
    }
    async fn time_correct(&self) -> Result<()> {
        self.write_bin_frame::<BsInt>(
            -1,
            Duration::ZERO.as_micros() as u64,
            (self.start_time.elapsed().as_micros() as u64).into(),
        )
        .unwrap();

        Ok(())
    }

    async fn handle_incoming_msg(
        &self,
        msg: core::result::Result<Message, TungsteniteError>,
    ) -> Result<()> {
        match msg {
            Ok(Message::Text(json)) => {
                let messages: Vec<ServerMsg> = serde_json::from_str(&json).unwrap();

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
                            self.server_topics
                                .write()
                                .await
                                .insert(name.clone(), (id, r#type.clone()));

                            if let Some(pubuid) = pubuid {
                                (*self.client_server.write().await).insert(pubuid, id);
                                (*self.server_client.write().await).insert(id, pubuid);

                                debug!(
                                    "{name} ({type}): published successfully with topic id {id}"
                                );
                            } else {
                                debug!("{name} ({type}): announced with topic id {id}");
                            }
                        }
                        ServerMsg::Unannounce {
                            name,
                            id: server_id,
                        } => {
                            if let Some(pubuid) = self.server_client.read().await.get(&server_id) {
                                self.client_server.write().await.remove(pubuid);
                            }
                            self.server_client.write().await.remove(&server_id);

                            debug!("{name}: unannounced");
                        }
                        _ => unimplemented!(),
                    }
                }
            }
            Ok(Message::Binary(bin)) => match Self::read_bin_frame(bin.to_vec()) {
                Ok((topic_id, timestamp, data)) => {
                    trace!(
                        "received binary frame with topic_id {}, ts={}",
                        topic_id,
                        timestamp
                    );

                    if topic_id == -1 {
                        let curr_ts = self.start_time.elapsed();
                        if let Data::Int(BsInt::U64(pre_ts)) = data {
                            let rtt = curr_ts - Duration::from_micros(pre_ts);
                            *self.offset.write().await =
                                Duration::from_micros(timestamp) + (rtt / 2);
                        }
                    }

                    if let Some(value_tx) = self.value_tx.write().await.get(&(topic_id as i32)) {
                        if let Err(err) = value_tx.send((timestamp, data)) {
                            error!("failed to send value to subscriber: {err:?}");
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to parse binary frame: {}", err);
                }
            },
            Ok(msg) => warn!("unhandled incoming message: {msg:?}"),
            Err(TungsteniteError::ConnectionClosed)
            | Err(TungsteniteError::Io(_))
            | Err(TungsteniteError::AlreadyClosed)
            | Err(TungsteniteError::Protocol(_)) => {
                return Err(NtError::NeedReconnect);
            }
            Err(err) => error!("error reading incoming message: {err:?}"),
        }

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
    pub async fn publish<T: DataWrap>(&self, name: impl Into<String>) -> Result<NtTopic<T>> {
        let pubuid = self.next_id().await;
        let name = name.into();

        trace!("publishing {name} with pubuid {pubuid}");

        self.publish_::<T>(name.clone(), pubuid).await?;

        Ok(NtTopic {
            conn: &*self,
            name,
            pubuid,
            _marker: PhantomData,
        })
    }
    async fn publish_<T: DataWrap>(&self, name: String, pubuid: i32) -> Result<()> {
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

        self.c2s_tx
            .send(Message::Text(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))
            .unwrap();

        debug!(
            "{name} ({data_type}): publishing with pubuid {pubuid}",
            data_type = T::STRING.to_string()
        );

        Ok(())
    }

    /// Unpublish topic
    ///
    /// This method is typically called when an `NtTopic` is dropped.
    fn unpublish(&self, pubuid: i32) -> Result<()> {
        let buf = serde_json::to_string(&[ClientMsg::Unpublish { pubuid }])?;
        self.c2s_tx
            .send(Message::Text(buf.into()))
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
        self.c2s_tx
            .send(Message::Text(buf.into()))
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
        self.c2s_tx
            .send(Message::Text(buf.into()))
            .map_err(|e| NtError::SendError(e.to_string()))?;

        Ok(())
    }

    /// Read/parse a binary frame
    ///
    /// This method is used internally to process incoming data values for subscribed topics.
    ///
    /// Returns `(uid, timestamp, data)`
    fn read_bin_frame(buf: Vec<u8>) -> Result<(i32, u64, Data)> {
        let mut bytes = Bytes::new(&buf);
        let len = rmp::decode::read_array_len(&mut bytes)?;

        if len == 4 {
            let uid = rmp::decode::read_i32(&mut bytes)?;
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
    fn write_bin_frame<T: DataWrap>(&self, uid: i32, ts: u64, value: T) -> Result<()> {
        let mut buf = Vec::new();
        rmp::encode::write_array_len(&mut buf, 4)?;

        rmp::encode::write_i32(&mut buf, uid)?;
        rmp::encode::write_uint(&mut buf, ts)?;
        rmp::encode::write_u8(&mut buf, T::MSGPCK)?;
        T::encode(&mut buf, value).map_err(|_| {
            NtError::MessagePackError("Failed to encode value to MessagePack format.".to_string())
        })?;

        self.c2s_tx
            .send(Message::Binary(buf.into()))
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
            start_time: self.start_time.clone(),
            offset: self.offset.clone(),

            reconnect_tx: self.reconnect_tx.clone(),
            next_id: self.next_id.clone(),

            incoming_abort: self.incoming_abort.clone(),
            outgoing_abort: self.outgoing_abort.clone(),
            task_abort: self.task_abort.clone(),

            c2s_tx: self.c2s_tx.clone(),

            sock_wr: self.sock_wr.clone(),
            sock_rd: self.sock_rd.clone(),

            client_server: self.client_server.clone(),
            server_client: self.server_client.clone(),

            server_topics: self.server_topics.clone(),
            values: self.values.clone(),
            value_tx: self.value_tx.clone(),
        }
    }
}

/// A NetworkTables topic
///
/// This structure represents a published topic on the NetworkTables server. It allows you to set
/// the value of the topic. The topic is automatically unpublished when this structure is dropped.
pub struct NtTopic<'nt, T: DataWrap> {
    conn: &'nt NtConn,
    name: String,
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
        debug!(
            "{name} ({data_type}): set to {val:?}",
            name = self.name,
            data_type = T::STRING.to_string()
        );

        if !self
            .conn
            .client_server
            .read()
            .await
            .contains_key(&self.pubuid)
        {
            self.conn
                .publish_::<T>(self.name.clone(), self.pubuid)
                .await?;
        }

        trace!("writing binary frame");
        match (*self.conn).write_bin_frame(self.pubuid, 0, val) {
            Err(_) => {}
            _ => {}
        }

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
    pub async fn get(&self) -> Result<Option<watch::Receiver<(u64, Data)>>> {
        Ok(self.conn.values.read().await.get(&self.subuid).cloned())
    }
}
impl Drop for NtSubscription<'_> {
    fn drop(&mut self) {
        self.conn.unsubscribe(self.subuid).unwrap();
    }
}
