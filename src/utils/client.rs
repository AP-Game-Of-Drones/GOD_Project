pub mod chat_client;
pub mod web_browser;

pub trait Client: Sized + Send + Sync {}
