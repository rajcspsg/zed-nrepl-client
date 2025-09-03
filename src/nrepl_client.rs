use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct NreplClient {
    stream: TcpStream,
    session: Option<String>,
    read_timeout: Duration,
    write_timeout: Duration,
    port: u16,
}

impl Clone for NreplClient {
    fn clone(&self) -> Self {
        // Reconnect to the same host/port, session will be reset
        //let client = NreplClient::connect("127.0.0.1", self.port)
        //    .expect("Failed to clone NreplClient: could not reconnect");
        let tcp_stream = match self.stream.try_clone() {
            Ok(stream) => stream,
            Err(_) => TcpStream::connect(format!("127.0.0.1:{}", self.port))
                .expect("Couldn't connect to the server..."),
        };
        let client = NreplClient {
            stream: tcp_stream,
            session: self.session.clone(),
            read_timeout: self.read_timeout,
            write_timeout: self.write_timeout,
            port: self.port,
        };
        client
    }
}

#[derive(Debug)]
pub struct EvalResult {
    pub value: Option<String>,
    pub output: String,
    pub error: String,
    pub has_error: bool,
}

impl Default for EvalResult {
    fn default() -> Self {
        EvalResult {
            value: None,
            output: String::new(),
            error: String::new(),
            has_error: false,
        }
    }
}

#[derive(Debug)]
pub enum NreplError {
    ConnectionClosed,
    Timeout,
    ParseError(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for NreplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NreplError::ConnectionClosed => write!(f, "Connection closed by server"),
            NreplError::Timeout => write!(f, "Operation timed out"),
            NreplError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            NreplError::IoError(e) => write!(f, "IO error: {}", e),
            NreplError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for NreplError {}

impl From<std::io::Error> for NreplError {
    fn from(error: std::io::Error) -> Self {
        NreplError::IoError(error)
    }
}

impl NreplClient {
    pub fn connect(host: &str, port: u16) -> Result<Self, NreplError> {
        let stream = TcpStream::connect(format!("{}:{}", host, port))?;

        // Set timeouts
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;

        // Enable TCP keepalive to detect dropped connections
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            unsafe {
                let fd = stream.as_raw_fd();
                let keepalive: libc::c_int = 1;
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_KEEPALIVE,
                    &keepalive as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
            }
        }

        Ok(NreplClient {
            stream,
            session: None,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(10),
            port,
        })
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn set_timeouts(
        &mut self,
        read_timeout: Duration,
        write_timeout: Duration,
    ) -> Result<(), NreplError> {
        self.read_timeout = read_timeout;
        self.write_timeout = write_timeout;
        self.stream.set_read_timeout(Some(read_timeout))?;
        self.stream.set_write_timeout(Some(write_timeout))?;
        Ok(())
    }

    pub fn clone_session(&mut self) -> Result<String, NreplError> {
        let mut msg = HashMap::new();
        msg.insert(
            "op".to_string(),
            serde_bencode::value::Value::Bytes(b"clone".to_vec()),
        );
        msg.insert(
            "id".to_string(),
            serde_bencode::value::Value::Bytes(uuid::Uuid::new_v4().to_string().into_bytes()),
        );

        self.send_message(&msg)?;
        let response = self.read_message_with_timeout()?;

        if let Some(new_session) = response.get("new-session") {
            if let serde_bencode::value::Value::Bytes(session_bytes) = new_session {
                let session_id = String::from_utf8_lossy(session_bytes).to_string();
                self.session = Some(session_id.clone());
                return Ok(session_id);
            }
        }

        Err(NreplError::Other(
            "Failed to get session from clone response".to_string(),
        ))
    }

    pub fn eval(&mut self, code: &str) -> Result<EvalResult, NreplError> {
        self.eval_with_timeout(code, Duration::from_secs(60))
    }

    pub fn eval_with_timeout(
        &mut self,
        code: &str,
        timeout: Duration,
    ) -> Result<EvalResult, NreplError> {
        // Ensure we have a session
        if self.session.is_none() {
            self.clone_session()?;
        }

        let mut msg = HashMap::new();
        let eval_id = uuid::Uuid::new_v4().to_string();
        msg.insert(
            "op".to_string(),
            serde_bencode::value::Value::Bytes(b"eval".to_vec()),
        );
        msg.insert(
            "id".to_string(),
            serde_bencode::value::Value::Bytes(eval_id.clone().into_bytes()),
        );
        msg.insert(
            "code".to_string(),
            serde_bencode::value::Value::Bytes(code.as_bytes().to_vec()),
        );

        if let Some(session) = &self.session {
            msg.insert(
                "session".to_string(),
                serde_bencode::value::Value::Bytes(session.as_bytes().to_vec()),
            );
        }

        self.send_message(&msg)?;

        let mut result = EvalResult::default();
        let start_time = Instant::now();

        // Keep reading responses until we get "done" status or timeout
        loop {
            if start_time.elapsed() > timeout {
                return Err(NreplError::Timeout);
            }

            let response = match self.read_message_with_timeout() {
                Ok(resp) => resp,
                Err(NreplError::ConnectionClosed) => {
                    return Err(NreplError::ConnectionClosed);
                }
                Err(e) => return Err(e),
            };

            // Verify this response is for our request
            if let Some(serde_bencode::value::Value::Bytes(id_bytes)) = response.get("id") {
                let response_id = String::from_utf8_lossy(id_bytes);
                if response_id != eval_id {
                    continue; // Skip responses for other requests
                }
            }

            // Extract value
            if let Some(serde_bencode::value::Value::Bytes(value_bytes)) = response.get("value") {
                result.value = Some(String::from_utf8_lossy(value_bytes).to_string());
            }

            // Extract stdout
            if let Some(serde_bencode::value::Value::Bytes(out_bytes)) = response.get("out") {
                result.output.push_str(&String::from_utf8_lossy(out_bytes));
            }

            // Extract stderr
            if let Some(serde_bencode::value::Value::Bytes(err_bytes)) = response.get("err") {
                result.error.push_str(&String::from_utf8_lossy(err_bytes));
            }

            // Check status
            if let Some(serde_bencode::value::Value::List(status_list)) = response.get("status") {
                let mut is_done = false;
                for status_item in status_list {
                    if let serde_bencode::value::Value::Bytes(status_bytes) = status_item {
                        let status_str = String::from_utf8_lossy(status_bytes);
                        match status_str.as_ref() {
                            "done" => is_done = true,
                            "error" => result.has_error = true,
                            _ => {}
                        }
                    }
                }
                if is_done {
                    break;
                }
            }
        }

        Ok(result)
    }

    pub fn describe(&mut self) -> Result<HashMap<String, serde_bencode::value::Value>, NreplError> {
        let mut msg = HashMap::new();
        msg.insert(
            "op".to_string(),
            serde_bencode::value::Value::Bytes(b"describe".to_vec()),
        );
        msg.insert(
            "id".to_string(),
            serde_bencode::value::Value::Bytes(uuid::Uuid::new_v4().to_string().into_bytes()),
        );

        self.send_message(&msg)?;
        self.read_message_with_timeout()
    }

    pub fn interrupt(&mut self) -> Result<(), NreplError> {
        if let Some(session) = &self.session.clone() {
            let mut msg = HashMap::new();
            msg.insert(
                "op".to_string(),
                serde_bencode::value::Value::Bytes(b"interrupt".to_vec()),
            );
            msg.insert(
                "id".to_string(),
                serde_bencode::value::Value::Bytes(uuid::Uuid::new_v4().to_string().into_bytes()),
            );
            msg.insert(
                "session".to_string(),
                serde_bencode::value::Value::Bytes(session.as_bytes().to_vec()),
            );

            self.send_message(&msg)?;
            let _response = self.read_message_with_timeout()?;
        }
        Ok(())
    }

    pub fn is_connected(&mut self) -> bool {
        // Try to send a small describe message to check connection
        let mut msg = HashMap::new();
        msg.insert(
            "op".to_string(),
            serde_bencode::value::Value::Bytes(b"describe".to_vec()),
        );
        msg.insert(
            "id".to_string(),
            serde_bencode::value::Value::Bytes(uuid::Uuid::new_v4().to_string().into_bytes()),
        );

        match self.send_message(&msg) {
            Ok(_) => {
                // Try to read response
                match self.read_message_with_timeout() {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    fn send_message(
        &mut self,
        msg: &HashMap<String, serde_bencode::value::Value>,
    ) -> Result<(), NreplError> {
        let encoded =
            serde_bencode::to_bytes(msg).map_err(|e| NreplError::ParseError(e.to_string()))?;

        // Try to write with timeout
        match self.stream.write_all(&encoded) {
            Ok(_) => match self.stream.flush() {
                Ok(_) => Ok(()),
                Err(e) => match e.kind() {
                    ErrorKind::BrokenPipe
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::ConnectionReset => Err(NreplError::ConnectionClosed),
                    _ => Err(NreplError::IoError(e)),
                },
            },
            Err(e) => match e.kind() {
                ErrorKind::BrokenPipe
                | ErrorKind::ConnectionAborted
                | ErrorKind::ConnectionReset => Err(NreplError::ConnectionClosed),
                _ => Err(NreplError::IoError(e)),
            },
        }
    }

    fn read_message_with_timeout(
        &mut self,
    ) -> Result<HashMap<String, serde_bencode::value::Value>, NreplError> {
        let mut buffer = Vec::new();
        let mut temp_buffer = [0u8; 4096];
        let start_time = Instant::now();

        loop {
            if start_time.elapsed() > self.read_timeout {
                return Err(NreplError::Timeout);
            }

            match self.stream.read(&mut temp_buffer) {
                Ok(0) => {
                    // Connection closed
                    return Err(NreplError::ConnectionClosed);
                }
                Ok(n) => {
                    buffer.extend_from_slice(&temp_buffer[..n]);

                    // Try to decode what we have so far
                    match serde_bencode::from_bytes::<HashMap<String, serde_bencode::value::Value>>(
                        &buffer,
                    ) {
                        Ok(decoded) => return Ok(decoded),
                        Err(_) => {
                            // Need more data, continue reading
                            // But check if we have too much data (potential attack)
                            if buffer.len() > 1024 * 1024 {
                                // 1MB limit
                                return Err(NreplError::ParseError(
                                    "Message too large".to_string(),
                                ));
                            }
                            continue;
                        }
                    }
                }
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                        if !buffer.is_empty() {
                            // We have partial data, maybe try to decode it
                            if let Ok(decoded) = serde_bencode::from_bytes::<
                                HashMap<String, serde_bencode::value::Value>,
                            >(&buffer)
                            {
                                return Ok(decoded);
                            }
                        }
                        continue;
                    }
                    ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::ConnectionReset => {
                        return Err(NreplError::ConnectionClosed);
                    }
                    _ => return Err(NreplError::IoError(e)),
                },
            }
        }
    }

    pub fn close(&mut self) -> Result<(), NreplError> {
        if let Some(session) = &self.session.clone() {
            let mut msg = HashMap::new();
            msg.insert(
                "op".to_string(),
                serde_bencode::value::Value::Bytes(b"close".to_vec()),
            );
            msg.insert(
                "id".to_string(),
                serde_bencode::value::Value::Bytes(uuid::Uuid::new_v4().to_string().into_bytes()),
            );
            msg.insert(
                "session".to_string(),
                serde_bencode::value::Value::Bytes(session.as_bytes().to_vec()),
            );

            // Best effort - don't fail if close fails
            let _ = self.send_message(&msg);
            let _ = self.read_message_with_timeout();
            self.session = None;
        }
        Ok(())
    }
}

impl Drop for NreplClient {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
