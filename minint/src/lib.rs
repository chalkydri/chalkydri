use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::net::IpAddr;

use datatype::{Data, DataType, DataWrap};
use futures_util::{SinkExt, StreamExt};
use rmp::decode::Bytes;
use std::error::Error;
use tokio::sync::mpsc;

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
extern crate rmp;
extern crate tokio;
extern crate tokio_tungstenite;

mod datatype;
mod messages;
use messages::*;
use tokio::task::AbortHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{header, HeaderValue};
use tokio_tungstenite::tungstenite::Message;

/// A NetworkTables connection
pub struct NtConn<'nt> {
    next_id: i32,
    incoming_abort: AbortHandle,
    outgoing_abort: AbortHandle,
    c2s: mpsc::UnboundedSender<Message>,
    _marker: PhantomData<&'nt ()>,
}
impl<'nt> NtConn<'nt> {
    /// Connect to a NetworkTables server
    pub async fn new(
        server: impl Into<IpAddr>,
        client_ident: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
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
        let (mut sock, _) = tokio_tungstenite::connect_async(req).await.unwrap();
        let (mut sock_wr, mut sock_rd) = sock.split();

        // Spawn event loop to read and process incoming messages
        let incoming_abort = tokio::spawn(async move {
            loop {
                while let Some(buf) = sock_rd.next().await {
                    println!("recv");
                    match buf {
                        Ok(Message::Text(json)) => println!("json: {json:?}"),
                        Ok(Message::Binary(bin)) => println!("mpk: {bin:?}"),
                        Ok(msg) => warn!("unhandled incoming message: {msg:?}"),
                        Err(err) => error!("error reading incoming message: {err:?}"),
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .abort_handle();

        // Spawn event loop to send outgoing messages
        let outgoing_abort = tokio::spawn(async move {
            loop {
                while let Some(outgoing) = c2s_rx.recv().await {
                    sock_wr.send(outgoing).await.unwrap();
                    println!("sent");
                }
                tokio::task::yield_now().await;
            }
        })
        .abort_handle();

        Ok(Self {
            next_id: 0,
            c2s: c2s_tx,
            incoming_abort,
            outgoing_abort,
            _marker: PhantomData,
        })
    }

    /// Publish a topic
    ///
    /// The topic will be unpublished when the [NtTopic] is dropped.
    pub fn publish<T: DataType>(
        &mut self,
        name: impl Into<String>,
    ) -> Result<NtTopic<T>, Box<dyn Error>> {
        let pubuid = self.next_id;
        self.next_id += 1;

        let buf = serde_json::to_string(&[ClientMsg::Publish {
            pubuid,
            name: name.into(),
            r#type: T::DATATYPE_STRING.to_string(),
            properties: Some(PublishProps {
                persistent: Some(true),
                retained: Some(true),
            }),
        }])?;

        self.c2s.send(Message::Text(buf)).unwrap();

        Ok(NtTopic {
            conn: &*self,
            uid: pubuid,
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
    pub fn subscribe(&mut self, topics: &[&str]) -> Result<(), Box<dyn Error>> {
        let subuid = fastrand::i32(..);

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

        self.c2s.send(Message::binary(buf))?;

        Ok(())
    }

    /// Shutdown the connection
    pub fn stop(self) {
        self.incoming_abort.abort();
        self.outgoing_abort.abort();
    }
}

/// A NetworkTables topic
pub struct NtTopic<'nt, T: DataType> {
    conn: &'nt NtConn<'nt>,
    uid: i32,
    _marker: PhantomData<T>,
}
impl<T: DataType> NtTopic<'_, T> {
    pub fn set(&mut self, val: T) -> Result<(), Box<dyn Error>> {
        (*self.conn).write_bin_frame(self.uid, 0, val)?;

        Ok(())
    }
}
impl<T: DataType> Drop for NtTopic<'_, T> {
    fn drop(&mut self) {
        self.conn.unpublish(self.uid).unwrap();
    }
}


