use chrono::{DateTime, Utc};
use serenity::{
    async_trait,
    builder::CreateEmbed,
    client::Context,
    http::Http,
    model::{
        guild::Guild,
        id::UserId,
        misc::Mentionable,
        prelude::ChannelId,
        prelude::{GuildId as SerenityGuildId, User},
    },
    prelude::Mutex,
};
use songbird::{
    driver::Bitrate,
    input::{Metadata, Restartable},
    tracks::{PlayMode, TrackHandle},
    Call, Event, EventContext, EventHandler as VoiceEventHandler, Songbird,
};
use std::{ops::Sub, sync::Arc, time::Duration};

use tracing::{error, info};

use crate::model::discord_utils::*;

pub type Result<T> = std::result::Result<T, String>;

pub const DEFAULT_BITRATE: i32 = 128_000;
pub const NOT_IN_VOICE_ERROR: &str = "not in a voice channel";
pub const NOTHING_PLAYING_ERROR: &str = "nothing is playing";

pub async fn stop(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    let manager = _get_songbird(ctx).await;

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut e = default_embed();
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        queue.stop();

        e.title("Queue cleared.");
        return Ok(e);
    } else {
        return Err(NOT_IN_VOICE_ERROR.to_string());
    }
}

pub async fn skip(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    let manager = _get_songbird(ctx).await;

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut e = default_embed();
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        if let Err(_) = queue.skip() {
            return Err(NOTHING_PLAYING_ERROR.to_string());
        }
        e.title("Skipped Song");
        return Ok(e);
    } else {
        return Err(NOT_IN_VOICE_ERROR.to_string());
    }
}

pub async fn mute(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    let manager = _get_songbird(ctx).await;

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            return Err(NOT_IN_VOICE_ERROR.to_string());
        }
    };

    let mut handler = handler_lock.lock().await;

    let mut e = default_embed();
    if handler.is_mute() {
        if let Err(why) = handler.mute(false).await {
            return Err(format!("failed: {:?}", why));
        }

        e.title("Unmuted");
    } else {
        if let Err(why) = handler.mute(true).await {
            return Err(format!("failed: {:?}", why));
        } else {
            e.title("Now muted");
        }
    }
    Ok(e)
}
///
/// joins the the current channel of the message author
///
pub async fn join(ctx: &Context, guild: &Guild, author_id: UserId) -> Result<CreateEmbed> {
    // get guild id the message was send in
    let guild_id = guild.id;

    // find the voice channel of the author
    // None when author is in no channel
    let channel_id = guild
        .voice_states
        .get(&author_id)
        .and_then(|voice_state| voice_state.channel_id);

    // check if author is in any channel
    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            return Err(NOT_IN_VOICE_ERROR.to_string());
        }
    };

    let manager = _get_songbird(ctx).await;
    let (handle_lock, success) = manager.join(guild_id, connect_to).await;

    if let Ok(_channel) = success {
        let mut handle = handle_lock.lock().await;
        let bitrate = match handle.current_channel() {
            Some(channel) => _get_bitrate_for_channel(channel, &ctx.http).await,
            None => DEFAULT_BITRATE,
        };

        handle.set_bitrate(Bitrate::BitsPerSecond(bitrate.clone()));
        drop(handle);
        info!("setting bitrate {} for guild {}", bitrate, guild_id);
    } else {
        return Err("couldn't join the channel".to_string());
    }
    let mut e = default_embed();
    e.description(&format!("Joined {}", connect_to.mention()));
    Ok(e)
}

pub async fn play_pause(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    let manager = _get_songbird(ctx).await;

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let track = match &handler.queue().current() {
            Some(info) => info.clone(),
            None => {
                return Err(NOTHING_PLAYING_ERROR.to_string());
            }
        };
        drop(handler);

        let is_playing = match track.get_info().await {
            Ok(info) => info.playing == PlayMode::Play,
            Err(_) => false,
        };

        let mut e = default_embed();
        if is_playing {
            if let Err(why) = track.pause() {
                return Err(format!("couldn't pause track {:#?}", why));
            }
            e.title("Paused track");
        } else {
            if let Err(why) = track.play() {
                return Err(format!("couldn't resume track {:#?}", why));
            }
            e.title("Resumed track");
        }
        return Ok(e);
    } else {
        return Err(NOT_IN_VOICE_ERROR.to_string());
    }
}

pub async fn leave(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    let manager = _get_songbird(ctx).await;
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        let mut e = default_embed();
        if let Err(why) = manager.remove(guild_id).await {
            return Err(format!("failed: {:?}", why));
        }

        e.title("Left voice channel");
        return Ok(e);
    } else {
        return Err(NOT_IN_VOICE_ERROR.to_string());
    }
}
///
/// deafens bot
///
pub async fn deafen(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    // the songbird manager for the current call
    let manager = _get_songbird(ctx).await;
    // get the lock to the call
    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            return Err(NOT_IN_VOICE_ERROR.to_string());
        }
    };

    // lock the call
    let mut handler = handler_lock.lock().await;

    let mut e = default_embed();
    // check if the bot is already deafened
    if handler.is_deaf() {
        if let Err(why) = handler.deafen(false).await {
            return Err(format!("failed: {:?}", why));
        }

        e.title("Undeafened");
    } else {
        // deafen and let the user know if anything goes horribly wrong
        if let Err(why) = handler.deafen(true).await {
            return Err(format!("failed to deafen {:?}", why));
        }

        e.title("Deafened");
    }
    drop(handler);
    Ok(e)
}

///
/// queues the given link to the song queue of the current call
///
/// also does directly play if its the first song in queue and
/// basically sends the [`now_playing`] command to inform the user
/// that the song started playing
pub async fn play(
    ctx: &Context,
    guild: &Guild,
    chan_id: &ChannelId,
    author_id: &UserId,
    url: String,
) -> Result<CreateEmbed> {
    let guild_id = guild.id;
    // check if its actually a url
    // TODO: implement yt-search with search terms
    if !url.starts_with("http") {
        return Err("must provide a valid URL".to_string());
    }

    let manager = _get_songbird(ctx).await;

    // get the current call lock
    if let Some(handler_lock) = manager.get(guild_id) {
        // await the lock
        let mut handler = handler_lock.lock().await;

        // Here, we use lazy restartable sources to make sure that we don't pay
        // for decoding, playback on tracks which aren't actually live yet.
        let now = std::time::Instant::now();
        let source = match Restartable::ytdl(url, true).await {
            Ok(source) => source,
            Err(why) => {
                error!("Err starting source: {:?}", why);

                return Err("error sourcing ffmgep".to_string());
            }
        };
        info!(
            "Sourcing song took {}",
            humantime::format_duration(now.elapsed())
        );

        handler.enqueue_source(source.into());
        let queue = handler.queue().current_queue();
        drop(handler);
        // save to unwrap because we just queued a track
        let track = queue.last().expect("couldn't get handle of queued track");
        let time = chrono::Utc::now();
        let _ = track.add_event(
            Event::Delayed(Duration::from_millis(20)),
            NowPlaying {
                http: ctx.http.clone(),
                chan_id: chan_id.clone(),
                author_id: author_id.clone(),
                time,
            },
        );
        let mut e = default_embed();
        e.title(format!("Added Song to position {}", queue.len()));
        return Ok(e);
    } else {
        return Err(NOT_IN_VOICE_ERROR.to_string());
    }
}

///
/// basically sends a nice embed of the current playing song
///
pub async fn now_playing(ctx: &Context, guild_id: SerenityGuildId) -> Result<CreateEmbed> {
    let manager = _get_songbird(ctx).await;
    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        // get track
        if let Some(track) = handler.queue().current() {
            let mut e = default_embed();
            // field with the name as a hyperlink to the source
            e.field("Now Playing:", _hyperlink_song(track.metadata()), false);
            // field with a nice formatted duration
            let track_time = _duration_format(track.metadata().duration);
            let duration_string = match track.get_info().await {
                Ok(info) => format!("{}/{}", _duration_format(Some(info.position)), track_time),
                Err(_) => track_time,
            };
            e.field("Duration:", duration_string, false);
            // thumbnail url if it exists
            if let Some(image) = &track.metadata().thumbnail {
                e.image(image);
            }
            drop(handler);
            return Ok(e);
        } else {
            drop(handler);
            return Err(NOTHING_PLAYING_ERROR.to_string());
        }
    } else {
        return Err(NOT_IN_VOICE_ERROR.to_string());
    }
}

pub struct NowPlaying {
    chan_id: ChannelId,
    http: Arc<Http>,
    author_id: UserId,
    time: DateTime<Utc>,
}

#[async_trait]
impl VoiceEventHandler for NowPlaying {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(tracks) = ctx {
            let meta = match tracks.first() {
                Some((_, handle)) => {
                    let meta = handle.metadata().clone();
                    drop(handle);
                    meta
                }
                None => {
                    error!("couldn't get song");
                    return Some(Event::Cancel);
                }
            };

            let mut e = default_embed();
            if let Ok(user) = self.http.get_user(*self.author_id.as_u64()).await {
                _embed_song_with_author(&mut e, &meta, "Now Playing:", user, &self.time);
            } else {
                _embed_song(&mut e, &meta, "Now Playing:");
            }
            check_msg(
                self.chan_id
                    .send_message(&self.http, |m| {
                        m.set_embed(e);
                        m
                    })
                    .await,
            );
        }

        None
    }
}

fn _hyperlink_song(data: &Metadata) -> String {
    let mut finished_song = "[".to_string();
    if let Some(title) = &data.title {
        finished_song.push_str(title);
    }

    finished_song.push_str(" - ");

    if let Some(artist) = &data.artist {
        finished_song.push_str(artist);
    }

    finished_song.push_str("](");

    if let Some(link) = &data.source_url {
        finished_song.push_str(link);
    }

    finished_song.push_str(")");

    finished_song
}

fn _duration_format(duration: Option<Duration>) -> String {
    if let Some(d) = duration {
        if d != Duration::default() {
            return humantime::format_duration(
                // we don't want milliseconds
                d.sub(Duration::from_millis(d.subsec_millis().into())),
            )
            .to_string();
        }
    }
    "Live".to_string()
}

pub async fn _get_songbird(ctx: &Context) -> Arc<Songbird> {
    songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone()
}

async fn _get_current_song(handle_lock: Arc<Mutex<Call>>) -> Option<TrackHandle> {
    let handle = handle_lock.lock().await;
    handle.queue().current()
}

fn _embed_song(e: &mut CreateEmbed, track: &Metadata, field_title: &str) {
    e.field(field_title, _hyperlink_song(track), false);
    let track_time = _duration_format(track.duration);
    e.field("Duration:", track_time, false);
    // thumbnail url if it exists
    if let Some(image) = &track.thumbnail {
        e.image(image);
    }
}

fn _embed_song_with_author(
    e: &mut CreateEmbed,
    track: &Metadata,
    field_title: &str,
    user: User,
    time: &DateTime<Utc>,
) {
    e.footer(|f| {
        f.text(&format!("Song added by {}", user.tag()))
            .icon_url(user.avatar_url().unwrap_or(user.default_avatar_url()))
    });
    e.timestamp(time);

    _embed_song(e, track, field_title);
}

async fn _get_bitrate_for_channel(channel: songbird::id::ChannelId, http: &Arc<Http>) -> i32 {
    match http.get_channel(channel.0).await {
        Ok(ch) => match ch
            .guild()
            .map(|ch| ch.bitrate)
            .unwrap_or(Some(DEFAULT_BITRATE as u64))
        {
            Some(bitrate) => bitrate as i32,
            // returns default bitrate when it was a textchannel
            None => DEFAULT_BITRATE,
        },
        // what ever
        Err(_) => DEFAULT_BITRATE,
    }
}
