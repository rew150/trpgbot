use super::{Context, Error};

#[poise::command(slash_command)]
pub async fn ping(
    ctx: Context<'_>,
    #[description = "Ping message"] msg: String,
) -> Result<(), Error> {
    let pt = ctx
        .data()
        .ping
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let response = format!("Pong! {} (currently pinged {} time(s))", msg, pt + 1);
    ctx.say(response).await?;

    Ok(())
}
