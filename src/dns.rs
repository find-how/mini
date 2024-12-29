use std::net::Ipv4Addr;
use std::iter;
use hickory_proto::op::{MessageType, ResponseCode};
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use hickory_proto::rr::rdata::A;
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};

const DEFAULT_TLDS: &[&str] = &["test", "localhost"];

pub struct DnsHandler {
    address: Ipv4Addr,
    tlds: Vec<String>,
}

impl DnsHandler {
    pub fn new() -> Self {
        DnsHandler {
            address: Ipv4Addr::new(127, 0, 0, 1),
            tlds: DEFAULT_TLDS.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn is_supported_domain(&self, name: &Name) -> bool {
        if let Some(tld) = name.iter().last() {
            if let Ok(tld_str) = std::str::from_utf8(tld) {
                return self.tlds.iter().any(|t| t == tld_str);
            }
        }
        false
    }
}

#[async_trait::async_trait]
impl RequestHandler for DnsHandler {
    async fn handle_request<R: ResponseHandler>(&self, request: &Request, mut response_handle: R) -> ResponseInfo {
        let mut header = request.header().clone();
        header.set_message_type(MessageType::Response);

        if !self.is_supported_domain(&request.query().name().into()) {
            header.set_response_code(ResponseCode::NXDomain);
            let response = MessageResponseBuilder::from_message_request(request)
                .build(header.clone(), iter::empty(), iter::empty(), iter::empty(), iter::empty());
            return response_handle.send_response(response).await.expect("failed to send response");
        }

        let mut record = Record::new();
        record.set_name(request.query().name().clone().into());
        record.set_record_type(RecordType::A);
        record.set_dns_class(DNSClass::IN);
        record.set_ttl(300);
        record.set_data(Some(RData::A(A(self.address))));

        let answers = vec![record];
        let response = MessageResponseBuilder::from_message_request(request)
            .build(header.clone(), answers.iter(), iter::empty(), iter::empty(), iter::empty());
        response_handle.send_response(response).await.expect("failed to send response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::io;
    use std::sync::Mutex;
    use hickory_proto::op::{Message, MessageType, OpCode, Query, Header, ResponseCode};
    use hickory_proto::rr::Record;
    use hickory_proto::serialize::binary::{BinDecodable, BinEncodable, BinEncoder};
    use hickory_server::authority::{MessageRequest, MessageResponse};
    use hickory_server::server::{Protocol, ResponseHandler, ResponseInfo};

    #[derive(Clone)]
    struct StoredMessage {
        id: u16,
        message_type: MessageType,
        op_code: OpCode,
        response_code: ResponseCode,
    }

    #[derive(Clone)]
    struct MockResponseHandler {
        messages: Arc<Mutex<Vec<StoredMessage>>>,
    }

    impl MockResponseHandler {
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        async fn get_messages(&self) -> Vec<Message> {
            let stored = self.messages.lock().unwrap();
            stored.iter().map(|stored| {
                let mut message = Message::new();
                let mut header = Header::new();
                header.set_id(stored.id);
                header.set_message_type(stored.message_type);
                header.set_op_code(stored.op_code);
                header.set_response_code(stored.response_code);
                message.set_header(header);
                message
            }).collect()
        }
    }

    #[async_trait::async_trait]
    impl ResponseHandler for MockResponseHandler {
        async fn send_response<'a>(
            &mut self,
            response: MessageResponse<
                '_,
                'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
                impl Iterator<Item = &'a Record> + Send + 'a>,
        ) -> io::Result<ResponseInfo> {
            // Store the message fields
            let stored = StoredMessage {
                id: response.header().id(),
                message_type: response.header().message_type(),
                op_code: response.header().op_code(),
                response_code: response.header().response_code(),
            };

            // Safely append to the Vec
            let mut messages = self.messages.lock().unwrap();
            messages.push(stored);

            // Handle the response
            let mut bytes = Vec::with_capacity(512);
            let mut encoder = BinEncoder::new(&mut bytes);
            let info = response.destructive_emit(&mut encoder)?;
            Ok(info)
        }
    }

    #[tokio::test]
    async fn test_dns_handler_supported_domain() {
        let handler = DnsHandler::new();
        let addr: SocketAddr = "127.0.0.1:53".parse().unwrap();
        let name = Name::parse("example.test.", None).unwrap();
        let query = Query::query(name, RecordType::A);
        let mut message = Message::new();
        message.set_id(1);
        message.set_message_type(MessageType::Query);
        message.set_op_code(OpCode::Query);
        message.add_query(query);

        let message_bytes = message.to_bytes().unwrap();
        let message_req = MessageRequest::from_bytes(&message_bytes).unwrap();
        let request = Request::new(message_req, addr, Protocol::Udp);
        let response_handle = MockResponseHandler::new();
        let response_handle_clone = response_handle.clone();
        let response_info = handler.handle_request(&request, response_handle).await;
        assert_eq!(response_info.response_code(), ResponseCode::NoError);

        let messages = response_handle_clone.get_messages().await;
        assert_eq!(messages.len(), 1);
        let response = &messages[0];
        assert_eq!(response.header().response_code(), ResponseCode::NoError);
        assert_eq!(response.header().message_type(), MessageType::Response);
        assert_eq!(response.header().op_code(), OpCode::Query);
        assert_eq!(response.header().id(), 1);
    }

    #[tokio::test]
    async fn test_dns_handler_unsupported_domain() {
        let handler = DnsHandler::new();
        let addr: SocketAddr = "127.0.0.1:53".parse().unwrap();
        let name = Name::parse("example.com.", None).unwrap();
        let query = Query::query(name, RecordType::A);
        let mut message = Message::new();
        message.set_id(1);
        message.set_message_type(MessageType::Query);
        message.set_op_code(OpCode::Query);
        message.add_query(query);

        let message_bytes = message.to_bytes().unwrap();
        let message_req = MessageRequest::from_bytes(&message_bytes).unwrap();
        let request = Request::new(message_req, addr, Protocol::Udp);
        let response_handle = MockResponseHandler::new();
        let response_handle_clone = response_handle.clone();
        let response_info = handler.handle_request(&request, response_handle).await;
        assert_eq!(response_info.response_code(), ResponseCode::NXDomain);

        let messages = response_handle_clone.get_messages().await;
        assert_eq!(messages.len(), 1);
        let response = &messages[0];
        assert_eq!(response.response_code(), ResponseCode::NXDomain);
        assert_eq!(response.answer_count(), 0);
    }
}
