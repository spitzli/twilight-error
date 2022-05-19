//! Error handling utility for the Twilight ecosystem
//!
//! All of the crate's functionality is under [`Self::ErrorHandler`]

#![warn(clippy::cargo, clippy::nursery, clippy::pedantic, clippy::restriction)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::single_char_lifetime_names,
    clippy::missing_inline_in_public_items,
    clippy::implicit_return
)]

use std::{error::Error, fmt::Write, fs::OpenOptions, io::Write as _, path::Path};

use twilight_http::Client;
use twilight_model::id::{
    marker::{ChannelMarker, WebhookMarker},
    Id,
};

/// The main struct to handle errors
pub struct ErrorHandler<'a> {
    /// Channel to create message in on error
    channel: Option<(&'a Client, Id<ChannelMarker>)>,
    /// Webhook to execute on error
    webhook: Option<(&'a Client, Id<WebhookMarker>, &'a str)>,
    /// File to append to on error
    file: Option<&'a Path>,
}

impl<'a> ErrorHandler<'a> {
    /// Make a handler that only prints errors to [`io::stderr`]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            channel: None,
            webhook: None,
            file: None,
        }
    }

    /// Set the handler to create a message in the given channel on errors
    ///
    /// The channel can also be DM channel, such as the owner's
    pub fn channel(&mut self, client: &'a Client, channel_id: Id<ChannelMarker>) -> &mut Self {
        self.channel = Some((client, channel_id));
        self
    }

    /// Set the handler to execute the given webhook on errors
    pub fn webhook(
        &mut self,
        client: &'a Client,
        webhook_id: Id<WebhookMarker>,
        token: &'a str,
    ) -> &mut Self {
        self.webhook = Some((client, webhook_id, token));
        self
    }

    /// Set the file to append to on error
    ///
    /// The file will be created if it doesn't exist
    pub fn file(&mut self, path: &'a Path) -> &mut Self {
        self.file = Some(path);
        self
    }

    /// Handle an error
    ///
    /// - Prints the error message to [`io::stderr`]
    /// - If [`Self::channel`] was called, creates a message in the given
    ///   channel with the error message
    /// - If [`Self::webhook`] was called, executes the webhook with the error
    ///   message
    /// - If [`Self::file`] was called, appends the error message to the file
    ///
    /// If the error message isn't valid as a message or webhook content (if
    /// it's too long), the fallback message content saying an error occurred
    /// will be used instead
    ///
    /// Note that the fields are not set in a falling back manner, for example,
    /// if both [`Self::channel`] and [`Self::webhook`] are called, it both
    /// creates a message and executes the webhook
    ///
    /// If [`Self::channel`] or [`Self::webhook`] wasn't called, this method
    /// isn't actually `async`
    ///
    /// # Panics
    /// If the fallback message or webhook content is somehow invalid
    #[allow(unused_must_use, clippy::unwrap_used)]
    pub async fn handle(&self, error: impl Error + Send) {
        let mut error_message = format!("{error}");

        if let Some((client, channel_id)) = self.channel {
            if let Err(err) = client
                .create_message(channel_id)
                .content(&error_message)
                .unwrap_or_else(|_| {
                    {
                        client
                            .create_message(channel_id)
                            .content("An error occurred, check the `stderr` for more info")
                    }
                    .unwrap()
                })
                .exec()
                .await
            {
                write!(error_message, "\n\nFailed to create message: {err}");
            }
        }

        if let Some((client, webhook_id, token)) = self.webhook {
            if let Err(err) = client
                .execute_webhook(webhook_id, token)
                .content(&error_message)
                .unwrap_or_else(|_| {
                    client
                        .execute_webhook(webhook_id, token)
                        .content("An error occurred, check the `stderr` for more info")
                        .unwrap()
                })
                .exec()
                .await
            {
                write!(error_message, "\n\nFailed to execute webhook: {err}");
            }
        }

        if let Some(path) = self.file {
            if let Err(err) = OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .and_then(|mut file| file.write_all(error_message.as_ref()))
            {
                write!(error_message, "\n\nFailed to append to file: {err}");
            }
        }
    }
}
