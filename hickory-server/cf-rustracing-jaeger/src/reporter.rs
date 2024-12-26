//! Reporter to the [jaeger agent]
//!
//! [jaeger agent]: http://jaeger.readthedocs.io/en/latest/deployment/#agent
use crate::constants;
use crate::error;
use crate::span::FinishedSpan;
use crate::thrift::{agent, jaeger};
use crate::Result;
use cf_rustracing::tag::Tag;
use std::net::SocketAddr;
use thrift_codec::message::Message;
use thrift_codec::{BinaryEncode, CompactEncode};
use tokio::net::UdpSocket;

/// Reporter for the agent which accepts jaeger.thrift over compact thrift protocol.
#[derive(Debug)]
pub struct JaegerCompactReporter(JaegerReporter);
impl JaegerCompactReporter {
    /// Makes a new `JaegerCompactReporter` instance.
    pub async fn new(
        service_name: &str,
        agent_addr: SocketAddr,
        reporter_addr: SocketAddr,
    ) -> Result<Self> {
        let inner = JaegerReporter::new(service_name, agent_addr, reporter_addr).await?;

        Ok(JaegerCompactReporter(inner))
    }

    /// Makes a new [`JaegerCompactReporter`] with the provided UDP socket transport.
    pub fn new_with_transport(
        service_name: &str,
        agent_addr: SocketAddr,
        socket: UdpSocket,
    ) -> Result<Self> {
        let inner = JaegerReporter::new_with_transport(service_name, agent_addr, socket)?;

        Ok(JaegerCompactReporter(inner))
    }

    /// Adds `tag` to this service.
    pub fn add_service_tag(&mut self, tag: Tag) {
        self.0.add_service_tag(tag);
    }

    /// Reports `spans`.
    ///
    /// # Errors
    ///
    /// If it fails to encode `spans` to the thrift compact format (i.e., a bug of this crate),
    /// this method will return an error which has the kind `ErrorKind::InvalidInput`.
    ///
    /// If it fails to send the encoded binary to the jaeger agent via UDP,
    /// this method will return an error which has the kind `ErrorKind::Other`.
    pub async fn report(&self, spans: &[FinishedSpan]) -> Result<()> {
        self.0
            .report(spans, |message| {
                let mut bytes = Vec::new();
                message
                    .compact_encode(&mut bytes)
                    .map_err(error::from_thrift_error)?;
                Ok(bytes)
            })
            .await
    }
}

/// Reporter for the agent which accepts jaeger.thrift over binary thrift protocol.
#[derive(Debug)]
pub struct JaegerBinaryReporter(JaegerReporter);
impl JaegerBinaryReporter {
    /// Makes a new `JaegerBinaryReporter` instance.
    pub async fn new(
        service_name: &str,
        agent_addr: SocketAddr,
        reporter_addr: SocketAddr,
    ) -> Result<Self> {
        let inner = JaegerReporter::new(service_name, agent_addr, reporter_addr).await?;

        Ok(JaegerBinaryReporter(inner))
    }

    /// Makes a new `JaegerBinaryReporter` with the provided UDP socket transport.
    pub fn new_with_transport(
        service_name: &str,
        agent_addr: SocketAddr,
        socket: UdpSocket,
    ) -> Result<Self> {
        let inner = JaegerReporter::new_with_transport(service_name, agent_addr, socket)?;

        Ok(JaegerBinaryReporter(inner))
    }

    /// Adds `tag` to this service.
    pub fn add_service_tag(&mut self, tag: Tag) {
        self.0.add_service_tag(tag);
    }

    /// Reports `spans`.
    ///
    /// # Errors
    ///
    /// If it fails to encode `spans` to the thrift binary format (i.e., a bug of this crate),
    /// this method will return an error which has the kind `ErrorKind::InvalidInput`.
    ///
    /// If it fails to send the encoded binary to the jaeger agent via UDP,
    /// this method will return an error which has the kind `ErrorKind::Other`.
    pub async fn report(&self, spans: &[FinishedSpan]) -> Result<()> {
        self.0
            .report(spans, |message| {
                let mut bytes = Vec::new();
                message
                    .binary_encode(&mut bytes)
                    .map_err(error::from_thrift_error)?;
                Ok(bytes)
            })
            .await
    }
}

#[derive(Debug)]
struct JaegerReporter {
    socket: UdpSocket,
    agent_addr: SocketAddr,
    process: jaeger::Process,
}

impl JaegerReporter {
    async fn new(
        service_name: &str,
        agent_addr: SocketAddr,
        reporter_addr: SocketAddr,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(reporter_addr)
            .await
            .map_err(error::from_io_error)?;

        Self::new_with_transport(service_name, agent_addr, socket)
    }

    fn new_with_transport(
        service_name: &str,
        agent_addr: SocketAddr,
        socket: UdpSocket,
    ) -> Result<Self> {
        let process = jaeger::Process {
            service_name: service_name.to_owned(),
            tags: Vec::new(),
        };

        let mut reporter = JaegerReporter {
            socket,
            agent_addr,
            process,
        };

        reporter.add_service_tag(Tag::new(
            constants::JAEGER_CLIENT_VERSION_TAG_KEY,
            constants::JAEGER_CLIENT_VERSION,
        ));

        if let Ok(Ok(hostname)) = hostname::get().map(|h| h.into_string()) {
            reporter.add_service_tag(Tag::new(constants::TRACER_HOSTNAME_TAG_KEY, hostname));
        }

        #[cfg(not(target_os = "android"))]
        if let Ok(local_ip_address) = local_ip_address::local_ip().map(|h| h.to_string()) {
            reporter.add_service_tag(Tag::new(constants::TRACER_IP_TAG_KEY, local_ip_address));
        }

        Ok(reporter)
    }

    fn add_service_tag(&mut self, tag: Tag) {
        self.process.tags.push((&tag).into());
    }

    async fn report<F>(&self, spans: &[FinishedSpan], encode: F) -> Result<()>
    where
        F: FnOnce(Message) -> Result<Vec<u8>>,
    {
        let batch = jaeger::Batch {
            process: self.process.clone(),
            spans: spans.iter().map(From::from).collect(),
        };

        let message = Message::from(agent::EmitBatchNotification { batch });
        let bytes = encode(message)?;

        self.socket
            .send_to(&bytes, self.agent_addr)
            .await
            .map_err(error::from_io_error)?;

        Ok(())
    }
}
