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

use datatype::{Data, DataType, DataWrap};
use messages::*;

use futures_util::{SinkExt, StreamExt};
use rmp::decode::Bytes;
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
pub struct NtConn<'nt> {
    next_id: Mutex<i32>,

    incoming_abort: AbortHandle,
    outgoing_abort: AbortHandle,

    c2s: mpsc::UnboundedSender<Message>,

    topics: Arc<Mutex<HashMap<i32, String>>>,
    topic_pubuids: Arc<Mutex<HashMap<i32, i32>>>,
    pubuid_topics: Arc<Mutex<HashMap<i32, i32>>>,

    _marker: PhantomData<&'nt ()>,
}
impl<'nt> NtConn<'nt> {
    /// Connect to a NetworkTables server
    pub async fn new(
        server: impl Into<IpAddr>,
        client_ident: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let topics = Arc::new(Mutex::new(HashMap::new()));
        let topic_pubuids = Arc::new(Mutex::new(HashMap::new()));
        let pubuid_topics = Arc::new(Mutex::new(HashMap::new()));

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

        let incoming_abort = {
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
                                            (*topics.lock().await).insert(id, name.clone());

                                            if let Some(pubuid) = pubuid {
                                                let mut pubuid_topics = pubuid_topics.lock().await;
                                                let mut topic_pubuids = topic_pubuids.lock().await;

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
                                            let mut topics = topics.lock().await;
                                            let topic_pubuids = topic_pubuids.lock().await;
                                            let mut pubuid_topics = pubuid_topics.lock().await;

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
                                println!("{bin:?}");
                            },
                            Ok(msg) => warn!("unhandled incoming message: {msg:?}"),
                            Err(err) => error!("error reading incoming message: {err:?}"),
                        }
                    }
                    tokio::task::yield_now().await;
                }
            })
            .abort_handle()
        };

        // Spawn event loop to send outgoing messages
        let outgoing_abort = tokio::spawn(async move {
            loop {
                while let Some(outgoing) = c2s_rx.recv().await {
                    sock_wr.send(outgoing).await.unwrap();
                }
                tokio::task::yield_now().await;
            }
        })
        .abort_handle();

        Ok(Self {
            next_id: Mutex::const_new(0),
            c2s: c2s_tx,

            topics,
            topic_pubuids,
            pubuid_topics,

            incoming_abort,
            outgoing_abort,

            _marker: PhantomData,
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
    pub async fn publish<T: DataType>(
        &self,
        name: impl Into<String>,
    ) -> Result<NtTopic<T>, Box<dyn Error>> {
        let pubuid = self.next_id().await;
        let name = name.into();

        let buf = serde_json::to_string(&[ClientMsg::Publish {
            pubuid,
            name: name.clone(),
            r#type: T::DATATYPE_STRING.to_string(),
            properties: Some(PublishProps {
                persistent: Some(true),
                retained: Some(true),
            }),
        }])?;

        self.c2s.send(Message::Text(buf)).unwrap();

        debug!(
            "{name} ({data_type}): publishing with pubuid {pubuid}",
            data_type = T::DATATYPE_STRING.to_string()
        );

        while !(*self.pubuid_topics.lock().await).contains_key(&pubuid) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(NtTopic {
            conn: &*self,
            pubuid,
            _marker: PhantomData,
        })
    }

    /// Unpublish topic
    fn unpublish(&self, pubuid: i32) -> Result<(), Box<dyn Error>> {
        let buf = serde_json::to_string(&[ClientMsg::Unpublish { pubuid }])?;
        self.c2s.send(Message::Text(buf))?;

        Ok(())
    }

    /// Subscribe to topic(s)
    pub async fn subscribe(&mut self, topics: &[&str]) -> Result<(), Box<dyn Error>> {
        let subuid = self.next_id().await;

        let buf = serde_json::to_string(&[ClientMsg::Subscribe {
            topics: topics.into_iter().map(|x| x.to_string()).collect(),
            subuid,
            options: BTreeMap::new(),
        }])?;
        self.c2s.send(Message::Text(buf))?;

        Ok(())
    }

    /// Unsubscribe from topic(s)
    pub fn unsubscribe(&mut self, subuid: i32) -> Result<(), Box<dyn Error>> {
        let buf = serde_json::to_string(&[ClientMsg::Unsubscribe { subuid }])?;
        self.c2s.send(Message::Text(buf))?;

        Ok(())
    }

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
    /// All [`topics`](NtTopic) must be dropped first.
    pub fn stop(self) {
        self.incoming_abort.abort();
        self.outgoing_abort.abort();
    }
}

/// A NetworkTables topic
///
/// Automatically unpublished when dropped.
pub struct NtTopic<'nt, T: DataType> {
    conn: &'nt NtConn<'nt>,
    pubuid: i32,
    _marker: PhantomData<T>,
}
impl<T: DataType + std::fmt::Debug> NtTopic<'_, T> {
    pub async fn set(&mut self, val: T) -> Result<(), Box<dyn Error>> {
        if let Some(id) = (*self.conn.pubuid_topics.lock().await).get(&self.pubuid) {
            if let Some(name) = (*self.conn.topics.lock().await).get(&id) {
                debug!(
                    "{name} ({data_type}): set to {val:?}",
                    data_type = T::DATATYPE_STRING.to_string()
                );
            }
        }

        (*self.conn).write_bin_frame(self.pubuid, 0, val)?;

        Ok(())
    }
}
impl<T: DataType> Drop for NtTopic<'_, T> {
    fn drop(&mut self) {
        self.conn.unpublish(self.pubuid).unwrap();
    }
}
