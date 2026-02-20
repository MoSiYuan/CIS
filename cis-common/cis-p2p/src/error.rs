use std::error::Error;

#[derive(Debug)]
pub enum P2pError {
    ConnectionFailed(String),
    NotConnected,
    SendError(String),
    ReceiveError(String),
    DiscoveryError(String),
}

impl std::fmt::Display for P2pError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2pError::ConnectionFailed(e) => write!(f, "Connection failed: {}", e),
            P2pError::NotConnected => write!(f, "Not connected to any peer"),
            P2pError::SendError(e) => write!(f, "Send error: {}", e),
            P2pError::ReceiveError(e) => write!(f, "Receive error: {}", e),
            P2pError::DiscoveryError(e) => write!(f, "Discovery error: {}", e),
        }
    }
}

impl Error for P2pError {}
