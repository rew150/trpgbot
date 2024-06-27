use lavalink_rs::{
    client::LavalinkClient,
    error::LavalinkError,
    model::events::{Events, TrackEnd},
    node::NodeBuilder,
    prelude::{NodeDistributionStrategy, TrackLoadData},
};
use songbird::{
    id::{ChannelId, GuildId},
    Call, Songbird,
};
use std::{sync::Arc, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum MusicRepoErr {
    #[error("MusicRepoErr/SongbirdJoinErr: {0}")]
    SongbirdJoinErr(#[from] songbird::error::JoinError),
    #[error("MusicRepoErr/LavalinkErr: {0}")]
    LavalinkErr(#[from] LavalinkError),
}

pub type Result<T, E = MusicRepoErr> = std::result::Result<T, E>;

pub struct MusicRepo {
    client: LavalinkClient,
}

impl MusicRepo {
    pub async fn new(nodes: Vec<NodeBuilder>) -> Self {
        Self {
            client: LavalinkClient::new(
                Events {
                    track_end: Some(on_track_end),
                    ..Default::default()
                },
                nodes,
                NodeDistributionStrategy::main_fallback(),
            )
            .await,
        }
    }

    pub async fn join(
        mng: Arc<Songbird>,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<(songbird::ConnectionInfo, Arc<tokio::sync::Mutex<Call>>)> {
        Ok(mng.join_gateway(guild_id, channel_id).await?)
    }

    pub async fn leave(mng: Arc<Songbird>, guild_id: GuildId) -> Result<()> {
        _ = mng.remove(guild_id).await?;
        Ok(())
    }

    pub async fn test(
        &self,
        mng: Arc<Songbird>,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<()> {
        let (conn, _) = Self::join(mng.clone(), guild_id, channel_id).await?;

        let player = self
            .client
            .create_player_context_with_data(guild_id, conn.clone(), PlayerData::new(mng.clone()))
            .await?;

        let t = self
            .client
            .load_tracks(guild_id, "https://youtu.be/dQw4w9WgXcQ")
            .await?;
        let tdata = match t.data.unwrap() {
            TrackLoadData::Track(t) => t,
            _ => panic!("not"),
        };
        dbg!(&tdata);
        _ = player.play_now(&tdata).await?;
        Ok(())
    }
}

struct PlayerData {
    mng: Arc<Songbird>,
}

impl PlayerData {
    fn new(mng: Arc<Songbird>) -> Arc<PlayerData> {
        Arc::new(Self { mng })
    }
}

#[lavalink_rs::hook]
async fn on_track_end(client: LavalinkClient, _session_id: String, track_end: &TrackEnd) {
    let Some(context) = client.get_player_context(track_end.guild_id) else {
        return;
    };

    let data = context.data::<PlayerData>().unwrap();
    if let Ok(count) = context.get_queue().get_count().await {
        if count == 0 {
            _ = context.close();
            tokio::time::sleep(Duration::from_secs(5)).await;
            _ = MusicRepo::leave(
                data.mng.clone(),
                GuildId(track_end.guild_id.0.try_into().unwrap()),
            )
            .await;
        }
    }
}
