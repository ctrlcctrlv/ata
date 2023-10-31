///	# ata²
///
///	 © 2023    Fredrick R. Brennan <copypaste@kittens.ph>
///	 © 2023    Rik Huijzer <t.h.huijzer@rug.nl>
///	 © 2023–   ATA Project Authors
///
///  Licensed under the Apache License, Version 2.0 (the "License");
///  you may _not_ use this file except in compliance with the License.
///  You may obtain a copy of the License at
///
///      http://www.apache.org/licenses/LICENSE-2.0
///
///  Unless required by applicable law or agreed to in writing, software
///  distributed under the License is distributed on an "AS IS" BASIS,
///  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
///  See the License for the specific language governing permissions and
///  limitations under the License.

#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

mod args;
mod config;
mod help;
mod prompt;

use ansi_colors::ColouredStr;
use clap::Parser as _;
use rustyline::{error::ReadlineError, Cmd, Editor, KeyCode, KeyEvent, Modifiers};

use std::fs::File;
use std::io::Read;

use std::fs;
use std::result::Result;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::args::Ata2;
use crate::config::Config;
use crate::prompt::print_error;

#[tokio::main]
pub async fn main() -> prompt::TokioResult<()> {
    init_logger();
    let flags: Ata2 = Ata2::parse();
    if flags.print_shortcuts {
        help::commands();
        return Ok(());
    }
    let filename = flags.config.location();
    if !filename.exists() {
        let v1_filename = flags.config.location_v1();
        if v1_filename.exists() {
            fs::create_dir_all(&config::default_path::<2>(None).parent().unwrap())
                .expect("Could not make configuration directory");
            fs::copy(&v1_filename, &filename).expect(&format!(
                "Failed to copy {} to {}",
                v1_filename.to_string_lossy(),
                filename.to_string_lossy()
            ));
            warn!(
                "{}",
                &format!(
                    "Copied old configuration file to ata¹'s location {}",
                    filename.to_string_lossy()
                ),
            );
        } else {
            help::missing_toml();
        }
    }
    let mut contents = String::new();
    File::open(filename)
        .unwrap()
        .read_to_string(&mut contents)
        .expect("Could not read configuration file");

    let config = Arc::new(Config::from(&contents));
    let config_clone = config.clone();
    let had_first_interrupt: AtomicBool = AtomicBool::new(false);
    config.validate().unwrap_or_else(|e| {
        error!("Config error!: {e}. Dying.");
        panic!()
    });

    let mut header = ColouredStr::new("Ask the Terminal Anything²\n\n");
    header.bold();

    if atty::is(atty::Stream::Stderr) {
        eprint!("{}", header);
    }

    if !flags.hide_config && !config.ui.hide_config && atty::is(atty::Stream::Stderr) {
        eprintln!("{config}");
    }
    let mut rl = Editor::<()>::new()?;
    if config.ui.multiline_insertions {
        if atty::is(atty::Stream::Stdin) {
            // Cmd::Newline inserts a newline, Cmd::AcceptLine accepts the line
            rl.bind_sequence(KeyEvent(KeyCode::Enter, Modifiers::NONE), Cmd::Newline);
            rl.bind_sequence(
                KeyEvent(KeyCode::Char('d'), Modifiers::CTRL),
                Cmd::AcceptLine,
            );
        }
    }
    if atty::is(atty::Stream::Stdin) {
        if rl.load_history(&config.history_file).is_err() {
            warn!("No history file found. Creating a new one.");
            File::create(&config.history_file).unwrap_or_else(|e| {
                error!("Could not create history file: {e}");
                warn!("Using /dev/null as history file.");
                File::open("/dev/null").unwrap()
            });
        }
    }
    let (tx, rx): (Sender<Option<String>>, Receiver<Option<String>>) = mpsc::channel();
    let is_running = Arc::new(AtomicBool::new(false));
    let is_running_clone = is_running.clone();
    let abort = Arc::new(AtomicBool::new(false));
    let abort_clone = abort.clone();

    let handle = thread::spawn(move || {
        let abort = abort_clone.clone();
        let is_running = is_running.clone();
        loop {
            let msg: Result<_, _> = rx.recv();
            match msg {
                Ok(Some(line)) => {
                    let mut retry = true;
                    let mut count = 1;
                    while retry {
                        let result = prompt::request(
                            abort.clone(),
                            is_running.clone(),
                            &config_clone,
                            line.to_string(),
                            count,
                        );
                        retry = match result {
                            Ok(retry) => retry,
                            Err(e) => {
                                eprintln!();
                                eprintln!();
                                let msg = format!("prompt::request failed with: {e}");
                                print_error(is_running.clone(), &msg);
                                false
                            }
                        };
                        count += 1;
                        if retry {
                            let duration = Duration::from_millis(500);
                            thread::sleep(duration);
                        } else {
                            break;
                        }
                    }
                }
                Ok(None) => {
                    abort.store(true, Ordering::SeqCst);
                    break;
                }
                Err(_) => {
                    abort.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
    });

    if atty::is(atty::Stream::Stdin) {
        prompt::print_prompt();
    }

    // If stdin is not a tty, we want to read once to the end of it and then exit.
    let mut already_read = false;
    let mut stdin = std::io::stdin();
    loop {
        // Using an empty prompt text because otherwise the user would
        // "see" that the prompt is ready again during response printing.
        // Also, the current readline is cleared in some cases by rustyline,
        // so being on a newline is the only way to avoid that.
        let readline = if atty::is(atty::Stream::Stdin) {
            rl.readline("")
        } else if !already_read {
            let mut buf = String::with_capacity(1024);
            stdin.read_to_string(&mut buf)?;
            already_read = true;
            Ok(buf)
        } else {
            Err(ReadlineError::Eof)
        };
        match readline {
            Ok(line) => {
                if is_running_clone.load(Ordering::SeqCst) {
                    abort.store(true, Ordering::SeqCst);
                }
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(line.as_str());
                tx.send(Some(line)).unwrap();
                had_first_interrupt.store(false, Ordering::Relaxed);
            }
            Err(ReadlineError::Interrupted) => {
                if is_running_clone.load(Ordering::SeqCst) {
                    abort.store(true, Ordering::SeqCst);
                } else {
                    if config.ui.double_ctrlc && !had_first_interrupt.load(Ordering::Relaxed) {
                        had_first_interrupt.store(true, Ordering::Relaxed);
                        eprintln!("\nPress Ctrl-C again to exit.");
                        thread::sleep(Duration::from_millis(100));
                        eprintln!();
                        prompt::print_prompt();
                        continue;
                    } else {
                        tx.send(None).unwrap();
                        break;
                    }
                }
            }
            Err(ReadlineError::Eof) => {
                tx.send(None).unwrap();
                break;
            }
            Err(err) => {
                eprintln!("{err:?}");
                tx.send(None).unwrap();
                break;
            }
        }
    }

    handle.join().unwrap();

    if atty::is(atty::Stream::Stdin) {
        rl.save_history(&config.history_file)
            .unwrap_or_else(|e| error!("Could not save history: {e}"));
        info!(
            "Saved history to {history_file}. Number of entries: {entries}",
            history_file = config.history_file.to_string_lossy(),
            entries = rl.history().len()
        );
    }

    Ok(())
}

fn init_logger() {
    let env = env_logger::Env::default().default_filter_or("warn");
    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();
}
