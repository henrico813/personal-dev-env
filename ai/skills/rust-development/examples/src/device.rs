//! Device connection states.

use std::path::{Path, PathBuf};

/// A device's connection status and the data belonging to that status.
#[derive(Debug)]
pub enum Connection {
    /// No connection has been established.
    Disconnected,
    /// A connection has been established on this port.
    Connected { port: PathBuf },
    /// The last connection attempt failed for this reason.
    Failed { reason: String },
}

impl Connection {
    /// Returns the port only while connected.
    pub fn port(&self) -> Option<&Path> {
        match self {
            Self::Connected { port } => Some(port.as_path()),
            Self::Disconnected | Self::Failed { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Connection;
    use std::path::{Path, PathBuf};

    #[test]
    fn connected_device_exposes_port() {
        let connection = Connection::Connected {
            port: PathBuf::from("device-port"),
        };

        assert_eq!(connection.port(), Some(Path::new("device-port")));
    }

    #[test]
    fn disconnected_device_has_no_port() {
        assert_eq!(Connection::Disconnected.port(), None);
    }
}
