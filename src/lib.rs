//! Error handling utility for the Twilight ecosystem
//!
//! All of the crate's functionality is under [`ErrorHandler`]

#![warn(clippy::cargo, clippy::nursery, clippy::pedantic, clippy::restriction)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::single_char_lifetime_names,
    clippy::missing_inline_in_public_items,
    clippy::implicit_return,
    clippy::pattern_type_mismatch
)]

use std::{
    fmt::{Display, Write},
    fs::OpenOptions,
    io::Write as _,
    path::PathBuf,
};

use twilight_http::Client;
use twilight_model::id::{
    marker::{ChannelMarker, WebhookMarker},
    Id,
};

/// The main struct to handle errors
pub struct ErrorHandler {
    /// Channel to create message in on error
    channel: Option<Id<ChannelMarker>>,
    /// Webhook to execute on error
    webhook: Option<(Id<WebhookMarker>, String)>,
    /// File to append to on error
    file: Option<PathBuf>,
}

/// The error message to fall back to if the previous error message isn't valid
/// as a webhook or message content (if it's too long)
pub const DEFAULT_ERROR_MESSAGE: &str = "An error occurred, check the `stderr` for more info";

impl ErrorHandler {
    /// Make a handler that only prints errors to [`std::io::stderr`]
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
    pub fn channel(&mut self, channel_id: Id<ChannelMarker>) -> &mut Self {
        self.channel = Some(channel_id);
        self
    }

    /// Set the handler to execute the given webhook on errors
    pub fn webhook(&mut self, webhook_id: Id<WebhookMarker>, token: String) -> &mut Self {
        self.webhook = Some((webhook_id, token));
        self
    }

    /// Set the file to append to on error
    ///
    /// The file will be created if it doesn't exist
    pub fn file(&mut self, path: PathBuf) -> &mut Self {
        self.file = Some(path);
        self
    }

    /// Handle an error
    ///
    /// Prefer [`Self::handle_sync`] if [`Self::channel`] or [`Self::webhook`]
    /// aren't set
    ///
    /// - Prints the error message to [`std::io::stderr`]
    /// - If [`Self::channel`] was called, creates a message in the given
    ///   channel with the error message or [`DEFAULT_ERROR_MESSAGE`]
    /// - If [`Self::webhook`] was called, executes the webhook with the error
    ///   message or [`DEFAULT_ERROR_MESSAGE`]
    /// - If [`Self::file`] was called, appends the error message to the file
    ///
    /// Note that the fields are not set in a falling back manner, for example,
    /// if both [`Self::channel`] and [`Self::webhook`] are called, it both
    /// creates a message and executes the webhook
    ///
    /// # Panics
    /// If the fallback message or webhook content is somehow invalid
    #[allow(clippy::unwrap_used, unused_must_use, clippy::print_stderr)]
    pub async fn handle(&self, http: &Client, error: impl Display + Send) {
        let mut error_message = format!("\n\n{error}");

        self.maybe_create_message(http, &mut error_message).await;
        self.maybe_execute_webhook(http, &mut error_message).await;
        self.maybe_append_error(&mut error_message);

        eprintln!("{error_message}");
    }

    /// Handle an error, ignoring [`Self::channel`] and [`Self::webhook`]
    ///
    /// Prefer this if you've only set [`Self::file`]
    #[allow(clippy::print_stderr)]
    pub fn handle_sync(&self, error: impl Display) {
        let mut error_message = format!("\n\n{error}");

        self.maybe_append_error(&mut error_message);

        eprintln!("{error_message}");
    }

    /// Tries to create a message with the given error message or
    /// [`DEFAULT_ERROR_MESSAGE`], writing the returned error to the error
    /// message
    #[allow(unused_must_use, clippy::unwrap_used)]
    async fn maybe_create_message(&self, http: &Client, error_message: &mut String) {
        if let Some(channel_id) = self.channel {
            if let Err(err) = http
                .create_message(channel_id)
                .content(error_message)
                .unwrap_or_else(|_| {
                    {
                        http.create_message(channel_id)
                            .content(DEFAULT_ERROR_MESSAGE)
                    }
                    .unwrap()
                })
                .exec()
                .await
            {
                write!(error_message, "\n\nFailed to create message: {err}");
            }
        }
    }

    /// Tries to execute the webhook with the given error message or
    /// [`DEFAULT_ERROR_MESSAGE`], writing the returned error to the error
    /// message
    #[allow(unused_must_use, clippy::unwrap_used)]
    async fn maybe_execute_webhook(&self, http: &Client, error_message: &mut String) {
        if let Some((webhook_id, token)) = &self.webhook {
            if let Err(err) = http
                .execute_webhook(*webhook_id, token)
                .content(error_message)
                .unwrap_or_else(|_| {
                    http.execute_webhook(*webhook_id, token)
                        .content(DEFAULT_ERROR_MESSAGE)
                        .unwrap()
                })
                .exec()
                .await
            {
                write!(error_message, "\n\nFailed to execute webhook: {err}");
            }
        }
    }

    /// Tries to append the given error message to the path, writing the
    /// returned error to the error message
    #[allow(unused_must_use)]
    fn maybe_append_error(&self, error_message: &mut String) {
        if let Some(path) = &self.file {
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
