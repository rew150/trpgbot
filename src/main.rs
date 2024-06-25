mod commands;
mod repo;

use std::{str::FromStr, sync::Arc, time::Duration};

use commands::Data;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use repo::nist_beacon::NistBeaconRepo;
use sea_orm::{ConnectOptions, Database};
use serde::Deserialize;
use tracing_subscriber::filter::LevelFilter;

use poise::serenity_prelude as serenity;

pub type ArcMutex<T> = Arc<tokio::sync::Mutex<T>>;
/// Wrap `Arc::new(Mutex::new())`
#[inline(always)]
pub fn arcmutex<T>(t: T) -> ArcMutex<T> {
    Arc::new(tokio::sync::Mutex::new(t))
}

#[derive(Debug, Deserialize)]
struct Config {
    discord_token: String,
    sqlite_conn: String,
    log_level: String,

    guild_ids: Vec<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Send + Sync + std::error::Error>> {
    let conf: Config = Figment::new()
        .merge(Toml::file("config/config.toml"))
        .extract()?;

    dbg!(&conf);

    let tracing_level = LevelFilter::from_str(&conf.log_level)?;

    tracing_subscriber::fmt()
        .with_max_level(tracing_level)
        .with_test_writer()
        .with_file(true)
        .with_line_number(true)
        .init();

    let mut opt = ConnectOptions::new(&conf.sqlite_conn);
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(false);

    let db = Database::connect(opt).await?;

    db.ping().await?;

    let repo: Arc<NistBeaconRepo> = Arc::new(NistBeaconRepo::new(db));

    let token = &conf.discord_token;
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![commands::ping(), commands::roll()],
            ..Default::default()
        })
        .setup({
            let guild_ids = conf.guild_ids;
            move |ctx, _ready, framework| {
                Box::pin(async move {
                    for gid in guild_ids {
                        poise::builtins::register_in_guild(
                            ctx,
                            &framework.options().commands,
                            serenity::GuildId::new(gid),
                        )
                        .await?;
                    }
                    Ok(Data::new(repo))
                })
            }
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}
