use std::collections::BTreeMap;

use serde::{ser::SerializeMap, Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "lowercase", tag = "method", content = "params")]
pub enum ClientMsg {
    /// Publish Request Message
    ///
    /// Sent from a client to the server to indicate the client wants to start publishing values at the given topic. The server shall respond with a Topic Announcement Message ([Announce]), even if the topic was previously announced. The client can start publishing data values via MessagePack messages immediately after sending this message, but the messages will be ignored by the server if the publisher data type does not match the topic data type.
    Publish {
        /// Publish name
        ///
        /// The topic name being published
        name: String,
        /// Publisher UID
        ///
        /// A client-generated unique identifier for this publisher. Use the same UID later to unpublish. This is also the identifier that the client will use in MessagePack messages for this topic.
        pubuid: i32,
        /// Type of data
        ///
        /// The requested data type (as a string).
        ///
        /// If the topic is newly created (e.g. there are no other publishers) this sets the value type. If the topic was previously published, this is ignored. The [Announce] message contains the actual topic value type that the client shall use when publishing values.
        ///
        /// Implementations should indicate an error if the user tries to publish an incompatible type to that already set for the topic.
        r#type: String,
        /// Properties
        ///
        /// Initial topic properties.
        ///
        /// If the topic is newly created (e.g. there are no other publishers) this sets the topic properties. If the topic was previously published, this is ignored. The [Announce] message contains the actual topic properties. Clients can use the [SetProperties] message to change properties after topic creation.
        properties: Option<PublishProps>,
    },
    /// Publish Release Message
    ///
    /// Sent from a client to the server to indicate the client wants to stop publishing values for the given topic and publisher. The client should stop publishing data value updates via binary MessagePack messages for this publisher prior to sending this message.
    ///
    /// When there are no remaining publishers for a non-persistent topic, the server shall delete the topic and send a Topic Removed Message ([Unannounce]) to all clients who have been sent a previous Topic Announcement Message ([Announce]) for the topic.
    Unpublish {
        /// Publisher UID
        ///
        /// The same unique identifier passed to the [Publish] message
        pubuid: i32,
    },

    /// Set Properties Message
    ///
    /// Sent from a client to the server to change properties (see Properties) for a given topic. The server will send a corresponding Properties Update Message ([Properties]) to all subscribers to the topic (if the topic is published). This message shall be ignored by the server if the topic is not published.
    ///
    /// If a property is not included in the update map, its value is not changed. If a property is provided in the update map with a value of null, the property is deleted.
    SetProperties {
        /// Topic name
        name: String,
        /// Properties to update
        update: BTreeMap<String, String>,
    },

    /// Subscribe Message
    ///
    /// Sent from a client to the server to indicate the client wants to subscribe to value changes for the specified topics / groups of topics. The server shall send MessagePack messages containing the current values for any existing cached topics upon receipt, and continue sending MessagePack messages for future value changes. If a topic does not yet exist, no message is sent until it is created (via a publish), at which point a Topic Announcement Message ([Announce]) will be sent and MessagePack messages will automatically follow as they are published.
    ///
    /// Subscriptions may overlap; only one MessagePack message is sent per value change regardless of the number of subscriptions. Sending a subscribe message with the same subscription UID as a previous subscribe message results in updating the subscription (replacing the array of identifiers and updating any specified options).
    Subscribe {
        topics: Vec<String>,
        subuid: i32,
        options: BTreeMap<String, String>,
    },

    /// Unsubscribe Message
    ///
    /// Sent from a client to the server to indicate the client wants to stop subscribing to messages for the given subscription.
    Unsubscribe {
        /// Subscription UID
        ///
        /// The same unique identifier passed to the [Subscribe] message
        subuid: i32,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase", tag = "method", content = "params")]
pub enum ServerMsg {
    /// Topic Announcement Message
    ///
    /// The server shall send this message for each of the following conditions:
    ///  - To all clients subscribed to a matching prefix when a topic is created
    ///  - To a client in response to an Publish Request Message (publish) from that client
    Announce {
        /// Topic name
        name: String,
        /// Topic ID
        ///
        /// The identifier that the server will use in MessagePack messages for this topic
        id: i32,
        /// Data type
        ///
        /// The data type for the topic (as a string)
        r#type: String,
        /// Publisher UID
        ///
        /// If this message was sent in response to a publish message, the Publisher UID provided in that message. Otherwise absent.
        pubuid: Option<i32>,
        /// Properties
        ///
        /// Topic Properties
        properties: BTreeMap<String, bool>,
    },

    /// Topic Removed Message
    ///
    /// The server shall send this message when a previously announced (via a Topic Announcement Message ([Announce]) topic is deleted.
    Unannounce {
        /// Topic name
        name: String,
        /// Topic ID
        ///
        /// The identifier that the server was using for value updates
        id: i32,
    },

    /// Properties Update Message
    ///
    /// The server shall send this message when a previously announced (via a Topic Announcement Message ([Announce]) topic has its properties changed (via Set Properties Message ([SetProperties]).
    Properties {
        /// Topic name
        name: String,
        /// Acknowledgement
        ///
        /// True if this message is in response to a setproperties message from the same client. Otherwise absent.
        ack: bool,
    },
}

#[derive(Serialize)]
pub struct PublishProps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained: Option<bool>,
}
