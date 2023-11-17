//! # ata² — Ask the Terminal Anything²
//!
//!	 © 2023    Fredrick R. Brennan <copypaste@kittens.ph>
//!	 © 2023    Rik Huijzer <t.h.huijzer@rug.nl>
//!	 © 2023–   ATA Project Authors
//!
//!  Licensed under the Apache License, Version 2.0 (the "License");
//!  you may _not_ use this file except in compliance with the License.
//!  You may obtain a copy of the License at
//!
//!      http://www.apache.org/licenses/LICENSE-2.0
//!
//!  Unless required by applicable law or agreed to in writing, software
//!  distributed under the License is distributed on an "AS IS" BASIS,
//!  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//!  See the License for the specific language governing permissions and
//!  limitations under the License.
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod args;
pub use crate::args::Ata2;
mod config;
pub use crate::config::Config;
mod help;
mod prompt;
use crate::prompt::load_conversation;
mod readline;
mod state;
pub use crate::state::*;

use ansi_colors::ColouredStr;
use futures_util::future::FutureExt as _;
use futures_util::task::Context;
use futures_util::task::Poll;

use std::error::Error;
use std::fs::File;

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub type TokioResult<S = dyn Send + Sync, E = Box<dyn Error + Send + Sync>> = Result<S, E>;
#[tokio::main]
pub async fn main() -> TokioResult<()> {
    init_logger();
    if FLAGS.load.is_some() {
        load_conversation(FLAGS.load.as_ref().unwrap()).await?;
    }
    let mut rl = readline::Readline::new();
    if EXIT.load(Ordering::SeqCst) {
        return Ok(());
    }
    let config = CONFIGURATION.clone();
    config.validate().unwrap_or_else(|e| {
        error!("Config error!: {e}. Dying.");
        panic!()
    });

    let mut header = ColouredStr::new("Ask the Terminal Anything²\n\n");
    header.bold();

    if atty::is(atty::Stream::Stderr) {
        eprint!("{}", header);
    }

    if !FLAGS.hide_config && !config.ui.hide_config && atty::is(atty::Stream::Stderr) {
        eprintln!("{config}");
    }
    if atty::is(atty::Stream::Stdin) && config.ui.save_history {
        if rl.load_history().await.is_err() {
            warn!("No history file found. Creating a new one.");
            File::create(&config.ui.history_file).unwrap_or_else(|e| {
                error!("Could not create history file: {e}");
                warn!("Using /dev/null as history file.");
                File::open("/dev/null").unwrap()
            });
        }
    }
    rl.enable_multiline().await;
    rl.enable_request_save().await;
    // use tokio asynchronous message queue
    let (tx, mut rx): (tokio::sync::mpsc::Sender<Option<String>>, _) =
        tokio::sync::mpsc::channel(1);

    let handle = tokio::spawn(async move {
        let n_pending_debug_log_notices = Arc::new(AtomicUsize::new(0));
        loop {
            let msg = Box::pin(rx.recv()).poll_unpin(&mut Context::from_waker(
                futures_util::task::noop_waker_ref(),
            ));
            match msg {
                Poll::Ready(Some(Some(line))) => {
                    let result = prompt::request(line.to_string(), 0).await;
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            error!("failed to request: {e}");
                        }
                    }
                    n_pending_debug_log_notices.store(0, Ordering::SeqCst);
                }
                Poll::Ready(Some(None)) => {
                    n_pending_debug_log_notices.store(0, Ordering::SeqCst);
                    info!("Got None in API request loop, exiting");
                    break;
                }
                Poll::Ready(None) | Poll::Pending => {
                    // All the next 20 or so lines are just for debug logging…
                    {
                        let n = n_pending_debug_log_notices.fetch_add(1, Ordering::SeqCst);
                        static MAX_PENDING_DEBUG_LOG_NOTICES: usize = 10;
                        macro_rules! PENDING_LOOP_MSG {
                            () => {
                                "Got pending in API request loop, waiting 10ms ({n}/{max})"
                            };
                            ($msg:expr) => {
                                concat!(
                                    "Got pending in API request loop, waiting 10ms ({n}/{max}): ",
                                    $msg
                                )
                            };
                        }
                        if n <= MAX_PENDING_DEBUG_LOG_NOTICES {
                            debug!(
                                PENDING_LOOP_MSG!(),
                                n = n,
                                max = MAX_PENDING_DEBUG_LOG_NOTICES
                            );
                        } else if n == 11 {
                            debug!(PENDING_LOOP_MSG!("(will stop logging this message, but you can enable trace logging to see it again)"),
                                   n = n,
                                   max = MAX_PENDING_DEBUG_LOG_NOTICES);
                        } else if n >= 12 {
                            trace!(
                                PENDING_LOOP_MSG!(),
                                n = n,
                                max = MAX_PENDING_DEBUG_LOG_NOTICES
                            );
                        }
                    }
                    // …and now we're done with debug logging.
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                }
            }
        }
    });

    let readline_handle = rl.handle(tx).await;

    tokio::select! {
        _ = readline_handle => {
            info!("Readline died");
        }
        _ = handle => {
            info!("API request loop died");
        }
    }

    if atty::is(atty::Stream::Stdin) && config.ui.save_history {
        rl.save_history().await?;
        info!(
            "Saved history to {history_file}. Number of entries: {entries}",
            history_file = config.ui.history_file.to_string_lossy(),
            entries = rl.history_len().await
        );
    }

    Ok(())
}

fn init_logger() {
    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();
}
