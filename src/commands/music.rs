use songbird::Songbird;
use std::sync::Arc;

use crate::repo::music::MusicRepo;

use super::{Context, Result};

#[poise::command(slash_command)]
pub async fn pingmusic(ctx: Context<'_>) -> Result<()> {
    let mng = get_songbird(ctx).await;
    let (channel_id, guild_id) = get_channel_and_guild_id(ctx).await?;

    ctx.data()
        .music_repo
        .test(mng, guild_id.into(), channel_id.into())
        .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<()> {
    let mng = get_songbird(ctx).await;
    let guild_id = ctx.guild().ok_or(anyhow::anyhow!("could't get guild"))?.id;

    MusicRepo::leave(mng, guild_id.into()).await?;

    Ok(())
}

pub async fn get_songbird(ctx: Context<'_>) -> Arc<Songbird> {
    songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird initialized")
}

pub async fn get_channel_and_guild_id(
    ctx: Context<'_>,
) -> Result<(
    poise::serenity_prelude::ChannelId,
    poise::serenity_prelude::GuildId,
)> {
    let guild = ctx.guild().ok_or(anyhow::anyhow!("could't get guild"))?;
    let guild_id = guild.id;
    let channel_id = guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id)
        .ok_or(anyhow::anyhow!("couldn't get channel id"))?;

    Ok((channel_id, guild_id))
}
