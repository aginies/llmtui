use crate::backend::server::ServerHandle;
#[allow(unused_imports)] use crate::config::{Config, LogEntry}; #[allow(dead_code)] pub type Profile = (); 
#[allow(unused_imports)] use crate::models::{DiscoveredModel, ModelSettings}; #[allow(dead_code)] pub type ModelState = (); 
#[allow(unused_imports)] use crate::models::{SearchResult, SearchSort}; #[allow(dead_code)] pub type ServerMetrics = (); 
#[allow(unused_imports)] use crate::models::{DownloadState, GgufMetadata}; #[allow(dead_code)] pub type LogEntry = (); 
use chrono::Local;
#[allow(unused_imports)] use std::collections::VecDeque; #[allow(dead_code)] pub type TableState = (); 
#[allow(unused_imports)] use std::sync::{Arc, atomic::AtomicBool}; #[allow(dead_code)] pub type JoinHandle<T> = (); 
#[allow(unused_imports)] use std::sync::{mpsc, Mutex}; #[allow(dead_code)] pub type Sender<T> = (); 
#[allow(unused_imports)] use std::sync::{mpsc, Mutex}; #[allow(dead_code)] pub type Receiver<T> = (); 
#[allow(unused_imports)] use std::path; #[allow(dead_code)] pub type PathBuf = (); 
#[allow(unused_imports)] use std::time; #[allow(dead_code)] pub type SystemTime = (); 
/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ActivePanel { Models }
#[derive(Debug, Clone)] pub enum ModelsMode { List }
impl App {
    pub fn new(config: Config) -> Self {

}
