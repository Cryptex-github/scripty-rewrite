use serenity::client::Context;
use serenity::model::webhook::Webhook;
use songbird::events::context_data::DisconnectReason;
use songbird::id::GuildId;
use songbird::model::CloseCode;
use std::borrow::Cow;
use std::sync::Arc;

pub async fn driver_disconnect(
    guild_id: GuildId,
    reason: Option<DisconnectReason>,
    ctx: Context,
    webhook: Arc<Webhook>,
) {
    debug!(?guild_id, "handler disconnected");
    let (should_reconnect, reason) = match reason {
        Some(DisconnectReason::AttemptDiscarded) => {
            warn!(?guild_id, "reconnection failed due to another request");
            (false, None)
        }
        Some(DisconnectReason::Internal) => {
            error!(?guild_id, "disconnected due to songbird internal error");
            (true, Some("library internal error".into()))
        }
        Some(DisconnectReason::Io) => {
            warn!(?guild_id, "host IO error caused disconnection");
            (true, Some("host IO error".into()))
        }
        Some(DisconnectReason::ProtocolViolation) => {
            error!(
                ?guild_id,
                "disconnected due to songbird and discord disagreeing on protocol"
            );
            (
                true,
                Some("library and discord disagreed on protocol".into()),
            )
        }
        Some(DisconnectReason::TimedOut) => {
            warn!(?guild_id, "timed out waiting for connection");
            (true, Some("timed out waiting for connection".into()))
        }
        Some(DisconnectReason::WsClosed(None)) => {
            debug!(?guild_id, "voice session WebSocket closed without reason");
            (
                true,
                Some("discord closed connection without reason".into()),
            )
        }
        Some(DisconnectReason::WsClosed(Some(code))) => check_ws_close_err(code, guild_id),
        Some(_) => {
            warn!(?guild_id, "disconnected for unknown reason");
            (true, Some("disconnected for unknown reason".into()))
        }
        None => {
            debug!("requested disconnection from {}", guild_id);
            (false, None)
        }
    };

    if should_reconnect {
        debug!(?guild_id, "scheduling reconnect");
        // retry connection in 30 seconds
        tokio::spawn(async move {
            debug!(?guild_id, "sleeping 30 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            debug!(?guild_id, "attempting reconnect");
            // TODO: spawn reconnection
        });
    }

    if let Some(reason) = reason {
        debug!(?guild_id, "giving user reason for disconnection");
        if let Err(e) = webhook
            .execute(ctx, false, |w| {
                w.content(format!(
                    "I had an issue ({}) and disconnected from the voice chat. {}",
                    reason,
                    if should_reconnect {
                        "I'll try reconnecting in 30 seconds."
                    } else {
                        ""
                    }
                ))
            })
            .await
        {
            debug!(
                ?guild_id,
                "failed to notify user about disconnection: {}", e
            );
        }
    }
}

fn check_ws_close_err(reason: CloseCode, guild_id: GuildId) -> (bool, Option<Cow<'static, str>>) {
    match reason {
        CloseCode::UnknownOpcode => {
            error!(?guild_id, "voice session WebSocket closed: unknown opcode");
            (
                true,
                Some("discord closed connection due to unknown opcode".into()),
            )
        }
        CloseCode::InvalidPayload => {
            error!(?guild_id, "voice session WebSocket closed: invalid payload");
            (
                true,
                Some("discord closed connection due to an invalid payload".into()),
            )
        }
        CloseCode::NotAuthenticated => {
            error!(
                ?guild_id,
                "voice session WebSocket closed: not authenticated"
            );
            (
                true,
                Some("discord closed connection due to not being authenticated".into()),
            )
        }
        CloseCode::AuthenticationFailed => {
            error!(
                ?guild_id,
                "voice session WebSocket closed: failed to authenticate"
            );
            (
                true,
                Some("discord closed connection due to failing to authenticate".into()),
            )
        }
        CloseCode::AlreadyAuthenticated => {
            error!(
                ?guild_id,
                "voice session WebSocket closed: already authenticated"
            );
            (
                true,
                Some("discord closed connection due to already being authenticated".into()),
            )
        }
        CloseCode::SessionInvalid => {
            error!(
                ?guild_id,
                "voice session WebSocket closed: session no longer valid"
            );
            (true, Some("discord invalidated session".into()))
        }
        CloseCode::SessionTimeout => {
            error!(
                ?guild_id,
                "voice session WebSocket closed: session timed out"
            );
            (true, Some("session timed out".into()))
        }
        CloseCode::ServerNotFound => {
            warn!(
                ?guild_id,
                "voice session WebSocket closed: server not found"
            );
            (true, Some("voice server couldn't be found".into()))
        }
        CloseCode::UnknownProtocol => {
            warn!(
                ?guild_id,
                "voice session WebSocket closed: protocol unrecognized"
            );
            (true, Some("discord didn't recognize protocol".into()))
        }
        CloseCode::Disconnected => {
            debug!(
                ?guild_id,
                "voice session WebSocket closed: kicked/removed/deleted from channel"
            );
            (false, None)
        }
        CloseCode::VoiceServerCrash => {
            warn!(
                ?guild_id,
                "voice session WebSocket closed: voice server crashed"
            );
            (true, Some("discord voice server crashed".into()))
        }
        CloseCode::UnknownEncryptionMode => {
            warn!(
                ?guild_id,
                "voice session WebSocket closed: encryption scheme unrecognized"
            );
            (
                true,
                Some("discord didn't recognize encryption scheme".into()),
            )
        }
    }
}
