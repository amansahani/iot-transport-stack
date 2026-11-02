use std::fmt;

/// RFC 9112 HTTP/1.1 Request and Response Abstractions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub host: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpRequest {
    pub fn new_get(path: &str, host: &str) -> Self {
        Self {
            method: "GET".to_string(),
            path: path.to_string(),
            host: host.to_string(),
            headers: vec![
                ("Host".to_string(), host.to_string()),
                ("User-Agent".to_string(), "IIIT-SriCity-IoT-Client/1.0".to_string()),
                ("Accept".to_string(), "application/json".to_string()),
                ("Connection".to_string(), "close".to_string()),
            ],
            body: String::new(),
        }
    }

    pub fn new_post_json(path: &str, host: &str, json_payload: &str) -> Self {
        Self {
            method: "POST".to_string(),
            path: path.to_string(),
            host: host.to_string(),
            headers: vec![
                ("Host".to_string(), host.to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Content-Length".to_string(), json_payload.len().to_string()),
                ("Connection".to_string(), "close".to_string()),
            ],
            body: json_payload.to_string(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut msg = format!("{} {} HTTP/1.1\r\n", self.method, self.path);
        for (k, v) in &self.headers {
            msg.push_str(&format!("{}: {}\r\n", k, v));
        }
        msg.push_str("\r\n");
        msg.push_str(&self.body);
        msg.into_bytes()
    }

    pub fn parse(raw: &[u8]) -> Result<Self, &'static str> {
        let text = std::str::from_utf8(raw).map_err(|_| "Invalid UTF-8 in HTTP request")?;
        let mut lines = text.split("\r\n");

        let request_line = lines.next().ok_or("Empty HTTP request")?;
        let mut parts = request_line.split_whitespace();
        let method = parts.next().ok_or("Missing HTTP method")?.to_string();
        let path = parts.next().ok_or("Missing HTTP path")?.to_string();

        let mut headers = Vec::new();
        let mut host = String::new();

        for line in lines.by_ref() {
            if line.is_empty() {
                break; // Header boundary
            }
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim().to_string();
                let val = v.trim().to_string();
                if key.eq_ignore_ascii_case("Host") {
                    host = val.clone();
                }
                headers.push((key, val));
            }
        }

        let body = lines.collect::<Vec<&str>>().join("\r\n");

        Ok(Self {
            method,
            path,
            host,
            headers,
            body,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn new_200_json(body: &str) -> Self {
        Self {
            status_code: 200,
            reason_phrase: "OK".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Content-Length".to_string(), body.len().to_string()),
                ("Server".to_string(), "Rust-IoT-Embedded-Server/1.0".to_string()),
                ("Connection".to_string(), "close".to_string()),
            ],
            body: body.to_string(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut msg = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.reason_phrase);
        for (k, v) in &self.headers {
            msg.push_str(&format!("{}: {}\r\n", k, v));
        }
        msg.push_str("\r\n");
        msg.push_str(&self.body);
        msg.into_bytes()
    }

    pub fn parse(raw: &[u8]) -> Result<Self, &'static str> {
        let text = std::str::from_utf8(raw).map_err(|_| "Invalid UTF-8 in HTTP response")?;
        let mut lines = text.split("\r\n");

        let status_line = lines.next().ok_or("Empty HTTP response")?;
        let mut parts = status_line.split_whitespace();
        let _http_ver = parts.next().ok_or("Missing HTTP version")?;
        let code_str = parts.next().ok_or("Missing status code")?;
        let status_code = code_str.parse::<u16>().map_err(|_| "Invalid status code")?;
        let reason_phrase = parts.collect::<Vec<&str>>().join(" ");

        let mut headers = Vec::new();
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }

        let body = lines.collect::<Vec<&str>>().join("\r\n");

        Ok(Self {
            status_code,
            reason_phrase,
            headers,
            body,
        })
    }
}

/// RFC 7252 Constrained Application Protocol (CoAP) Header (Alternative IoT protocol over UDP)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoapMessage {
    pub version: u8,
    pub type_field: u8, // 0 = Confirmable, 1 = Non-confirmable, 2 = ACK, 3 = Reset
    pub code: u8,       // 0.01 = GET, 0.02 = POST, 2.05 = Content
    pub message_id: u16,
    pub token: Vec<u8>,
    pub payload: Vec<u8>,
}

impl CoapMessage {
    pub fn new_telemetry_put(msg_id: u16, payload: &[u8]) -> Self {
        Self {
            version: 1,
            type_field: 0, // Confirmable
            code: 3,       // PUT (0.03)
            message_id: msg_id,
            token: vec![0xAB, 0xCD],
            payload: payload.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let first_byte = (self.version << 6) | (self.type_field << 4) | (self.token.len() as u8 & 0x0F);
        buf.push(first_byte);
        buf.push(self.code);
        buf.extend_from_slice(&self.message_id.to_be_bytes());
        buf.extend_from_slice(&self.token);
        if !self.payload.is_empty() {
            buf.push(0xFF); // Payload marker
            buf.extend_from_slice(&self.payload);
        }
        buf
    }

    pub fn parse(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < 4 {
            return Err("CoAP message too short");
        }
        let version = (buf[0] >> 6) & 0x03;
        let type_field = (buf[0] >> 4) & 0x03;
        let token_len = (buf[0] & 0x0F) as usize;
        let code = buf[1];
        let message_id = u16::from_be_bytes([buf[2], buf[3]]);

        let mut offset = 4;
        if buf.len() < offset + token_len {
            return Err("Truncated CoAP token");
        }
        let token = buf[offset..offset + token_len].to_vec();
        offset += token_len;

        let mut payload = Vec::new();
        if offset < buf.len() && buf[offset] == 0xFF {
            offset += 1;
            payload = buf[offset..].to_vec();
        }

        Ok(Self {
            version,
            type_field,
            code,
            message_id,
            token,
            payload,
        })
    }
}
