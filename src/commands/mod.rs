use std::sync::{atomic::AtomicU64, Arc};

mod music;
pub use music::{pingmusic, stop};
mod ping;
pub use ping::ping;
mod roll;
pub use roll::roll;

use crate::repo::{music::MusicRepo, nist_beacon::NistBeaconRepo};

pub struct Data {
    ping: AtomicU64,
    nist_repo: Arc<NistBeaconRepo>,
    music_repo: Arc<MusicRepo>,
}
impl Data {
    pub fn new(nist_repo: Arc<NistBeaconRepo>, music_repo: Arc<MusicRepo>) -> Self {
        Self {
            ping: AtomicU64::new(0),
            nist_repo,
            music_repo,
        }
    }
}

pub type Error = anyhow::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
