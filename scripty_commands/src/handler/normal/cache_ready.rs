use serenity::client::Context;
use serenity::model::id::GuildId;

const SIZE_OF_GUILD_ID: usize = std::mem::size_of::<GuildId>();

pub async fn cache_ready(_ctx: Context, guilds: Vec<GuildId>) {
    let guild_count = guilds.len();
    info!(
        "cache is primed, {} guilds in cache for {} bytes",
        guild_count,
        guild_count * SIZE_OF_GUILD_ID
    )
}
