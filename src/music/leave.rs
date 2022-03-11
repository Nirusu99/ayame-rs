use poise::serenity_prelude::Result;
use songbird::id::GuildId;
use tracing::error;

use super::MusicContext;

pub async fn leave<'a, U, E>(mtx: &MusicContext<'a, U, E>) -> Result<()>
where
    U: Send + Sync,
    E: Send + Sync,
{
    if let Some(songbird) = super::get(&mtx.ctx).await {
        if let Err(why) = songbird.remove(GuildId::from(mtx.guild_id.0)).await {
            error!("failed to leave channel {:?}", why);
        }
    }
    Ok(())
}
