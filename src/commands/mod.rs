use std::sync::{atomic::AtomicU64, Arc};

mod ping;
pub use ping::ping;
mod roll;
pub use roll::roll;

use crate::repo::nist_beacon::NistBeaconRepo;

pub struct Data {
    ping: AtomicU64,
    nist_repo: Arc<NistBeaconRepo>,
}
impl Data {
    pub fn new(nist_repo: Arc<NistBeaconRepo>) -> Self {
        Self {
            ping: AtomicU64::new(0),
            nist_repo,
        }
    }
}

pub type Error = anyhow::Error;
pub type Context<'a> = poise::Context<'a, Data, Error>;
