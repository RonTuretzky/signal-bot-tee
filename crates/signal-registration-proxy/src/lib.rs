//! Signal Registration Proxy - TEE-secured phone number registration service.
//!
//! This proxy sits in front of the Signal CLI REST API to:
//! - Allow self-service registration of new phone numbers
//! - Prevent re-registration attacks on already-registered numbers
//! - Persist registration state with TEE-encrypted storage

pub mod api;
pub mod auth;
pub mod config;
pub mod error;
pub mod registry;
pub mod signal;

pub use config::Config;
pub use error::ProxyError;
pub use registry::{PhoneNumberRecord, Registry, RegistrationStatus, Store};
pub use signal::SignalRegistrationClient;
