use crate::Data;
use poise::{Context, FrameworkError};
use scripty_audio_handler::JoinError;
use serenity::builder::CreateEmbed;
use serenity::model::channel::ChannelType;
use serenity::model::id::GuildId;
use std::borrow::Cow;
use std::fmt::Write;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Serenity(serenity::Error),
    InvalidChannelType {
        expected: ChannelType,
        got: ChannelType,
    },
    MissingWebhookToken,
    Db(sqlx::Error),
    ExpectedGuild,
    Join(JoinError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use self::Error::*;
        let res: Cow<str> = match self {
            Serenity(e) => format!("Discord/wrapper returned an error: {}", e).into(),
            InvalidChannelType { expected, got } => format!(
                "Got an invalid channel type {:?} when expected {:?}",
                got, expected
            )
            .into(),
            MissingWebhookToken => "webhook token was not sent by discord".into(),
            Db(e) => format!("Database returned an error: {:?}", e).into(),
            // _ => "an unknown error happened".into(),
            ExpectedGuild => "expected this to be in a guild".into(),
            Join(e) => format!("failed to join VC: {}", e).into(),
        };
        f.write_str(res.as_ref())
    }
}

impl std::error::Error for Error {}

impl From<serenity::Error> for Error {
    #[inline]
    fn from(e: serenity::Error) -> Self {
        Self::Serenity(e)
    }
}

impl From<sqlx::Error> for Error {
    #[inline]
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

impl From<scripty_audio_handler::Error> for Error {
    #[inline]
    fn from(e: scripty_audio_handler::Error) -> Self {
        match e {
            scripty_audio_handler::Error::Join(e) => Self::Join(e),
            scripty_audio_handler::Error::Database(e) => Self::Db(e),
            scripty_audio_handler::Error::Serenity(e) => Self::Serenity(e),
        }
    }
}

pub async fn on_error(error: FrameworkError<'_, Data, crate::Error>) {
    info!("handling error event");
    #[allow(unreachable_patterns)]
    match error {
        FrameworkError::Setup { error } => panic!("error during bot init: {}", error),
        FrameworkError::Listener { error, event, .. } => {
            error!("error in listener for event {}: {}", event.name(), error)
        }
        FrameworkError::Command { error, ctx } => {
            let cmd_name = &ctx.command().qualified_name;

            send_err_msg(
                ctx,
                format!("An error happened while processing {}", cmd_name),
                format!(
                    "```\n{:?}\n```\nThis has been automatically reported. \
                        Please do not attempt to repeatedly use this command.",
                    error
                ),
            )
            .await;

            // cache the cache
            let cache = ctx.discord().cache.clone();

            let guild_id = ctx.guild_id().unwrap_or(GuildId(0));
            let guild_name = cache
                .guild(guild_id)
                .map_or_else(|| "unknown guild".to_string(), |g| g.name.clone());

            let channel_id = ctx.channel_id();

            let author = ctx.author();
            let author_id = author.id;
            let author_name = &author.name;

            error!(
                %guild_id, %guild_name, %channel_id, %author, %author_id, %author_name, %cmd_name,
                "error encountered while running command: {}", error
            )
        }
        FrameworkError::ArgumentParse { error, input, ctx } => {
            send_err_msg(
                ctx,
                format!(
                    "Invalid arguments while parsing {}",
                    ctx.command().qualified_name
                ),
                match input {
                    Some(input) => format!("Failed to parse `{}` because `{}`", input, error),
                    None => format!("{}", error),
                },
            )
            .await;
        }
        FrameworkError::CommandStructureMismatch { description, ctx } => {
            let mut root_embed = CreateEmbed::default();

            let mut args = String::new();
            for param in &ctx.command.parameters {
                if param.required {
                    write!(&mut args, "<{}> ", param.name)
                        .expect("failed to format string: this is a bug");
                } else {
                    write!(&mut args, "[{}] ", param.name)
                        .expect("failed to format string: this is a bug");
                }
            }

            root_embed
                .title(format!(
                    "Invalid structure from Discord while parsing {}",
                    ctx.command.qualified_name
                ))
                .color(serenity::utils::Color::from_rgb(255, 0, 0))
                .description(format!(
                    "{}\n\n\
                    **Note**: this is a Discord error\n\
                    The only fix for this is to wait for Discord to propagate slash commands, \
                    which can take up to one hour.\n\
                    If you do not want to wait this hour, you should use the prefix commands: \
                    run this command with `~{} {}`.",
                    description, ctx.command.qualified_name, args
                ));

            let response = ctx
                .interaction
                .channel_id()
                .send_message(&ctx.discord, |msg| {
                    msg.embed(|embed| {
                        *embed = root_embed.clone();
                        embed
                    })
                })
                .await;
            if let Err(e) = response {
                warn!("failed to send message while handling error: {}", e);
                let response = ctx
                    .interaction
                    .user()
                    .direct_message(ctx.discord, |msg| {
                        msg.embed(move |embed| {
                            *embed = root_embed;
                            embed
                        })
                    })
                    .await;
                if let Err(e) = response {
                    error!("failed to DM user while handling error: {}", e)
                }
            }
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
        } => {
            send_err_msg(
                ctx,
                format!("Cooldown hit on {}", ctx.command().qualified_name),
                format!(
                    "{:.2} seconds remaining on cooldown",
                    remaining_cooldown.as_secs_f32()
                ),
            )
            .await;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
        } => {
            send_err_msg(
                ctx,
                format!("I am missing perms to run {}", ctx.command().qualified_name),
                format!("Permissions missing: {}", missing_permissions),
            )
            .await;
        }
        FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
        } => {
            send_err_msg(
                ctx,
                format!(
                    "You are missing perms to run {}",
                    ctx.command().qualified_name
                ),
                match missing_permissions {
                    Some(p) => Cow::from(format!("Permissions missing: {}", p)),
                    None => Cow::from("I'm not sure what permissions you're missing."),
                },
            )
            .await;
        }
        FrameworkError::NotAnOwner { ctx } => {
            send_err_msg(
                ctx,
                format!(
                    "You are missing perms to run {}",
                    ctx.command().qualified_name
                ),
                "Not an owner of this bot",
            )
            .await;
        }
        FrameworkError::CommandCheckFailed { error, ctx } => {
            send_err_msg(
                ctx,
                format!("A precondition for {} failed", ctx.command().qualified_name),
                match error {
                    Some(e) => Cow::from(format!("{}", e)),
                    None => Cow::from("no reason provided"),
                },
            )
            .await;
        }
        _ => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("error while handling error: {}", e)
            }
        }
    }
}

async fn send_err_msg(
    ctx: Context<'_, Data, Error>,
    title: impl Into<String>,
    description: impl Into<String>,
) {
    let mut root_embed = CreateEmbed::default();
    root_embed
        .title(title)
        .color(serenity::utils::Color::from_rgb(255, 0, 0))
        .description(description);

    let response = ctx
        .send(|resp| {
            resp.embed(|embed| {
                embed.0 = root_embed.0.clone();
                embed
            })
        })
        .await;
    if let Err(e) = response {
        warn!("failed to send message while handling error: {}", e);
        let response = ctx
            .author()
            .direct_message(ctx.discord(), |msg| {
                msg.embed(move |embed| {
                    *embed = root_embed;
                    embed
                })
            })
            .await;
        if let Err(e) = response {
            error!("failed to DM user while handling error: {}", e)
        }
    }
}
