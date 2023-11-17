//! REPL
//!
//! # ata²
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

use ansi_colors::ColouredStr;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionResponseStreamMessage,
        CreateChatCompletionRequestArgs, FinishReason,
    },
    Client,
};
use atty;
use log::debug;
use tokio::sync::Mutex;
use tokio_stream::StreamExt as _;

use std::io::{self, Stderr, Stdout};
use std::io::{Read as _, Write as _};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::readline::{
    string_to_chat_completion_assistant_message, string_to_chat_completion_request_user_message,
};
use crate::TokioResult;
use crate::ABORT;
use crate::CONFIGURATION;
use crate::IS_RUNNING;

lazy_static! {
    static ref STDOUT: Stdout = io::stdout();
    static ref STDERR: Stderr = io::stderr();
    pub static ref CONVERSATION: Mutex<Vec<ChatCompletionRequestMessage>> = Mutex::new(vec![]);
}

pub async fn load_conversation<P: AsRef<std::path::Path>>(path: P) -> TokioResult<()> {
    let mut file = std::fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let lines = contents.split("\n").collect::<Vec<_>>();
    let mut conversation = CONVERSATION.lock().await;
    let loaded_conversation = serde_json::from_str::<Vec<ChatCompletionRequestMessage>>(
        &lines
            .into_iter()
            .filter(|o| !o.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    conversation.clear();
    conversation.extend(loaded_conversation);
    Ok(())
}

fn print_and_flush(text: &str) {
    print!("{text}");
    (&*STDOUT).flush().unwrap();
}

fn eprint_and_flush(text: &str) {
    eprint!("{text}");
    (&*STDERR).flush().unwrap();
}

fn eprint_bold(msg: &str) {
    if atty::is(atty::Stream::Stderr) {
        let mut bold = ColouredStr::new(msg);
        bold.bold();
        let bold = bold.to_string();
        eprint_and_flush(&bold.as_str());
    } else {
        eprint_and_flush(msg);
    }
}

pub fn print_prompt() {
    if atty::is(atty::Stream::Stderr) {
        eprint_bold("\nPrompt:\n");
    }
}

fn print_response_prompt() {
    if atty::is(atty::Stream::Stderr) {
        eprint_bold("\nResponse:\n");
    }
}

fn finish_prompt() {
    IS_RUNNING.store(false, Ordering::SeqCst);
    print_prompt();
}

fn print_error(msg: &str) {
    error!("{msg}");
    finish_prompt()
}

fn store_and_do_nothing(print_buffer: &mut Vec<String>, text: &str) -> String {
    print_buffer.push(text.to_string());
    "".to_string()
}

fn join_and_clear(print_buffer: &mut Vec<String>, text: &str) -> String {
    let from_buffer = print_buffer.join("");
    print_buffer.clear();
    let joined = format!("{from_buffer}{text}");
    joined.replace("\\n", "\n")
}

// Fixes cases where the model returns ["\", "n"] instead of ["\n"],
// which is interpreted as a newline in the OpenAI playground.
fn fix_newlines(print_buffer: &mut Vec<String>, text: &str) -> String {
    let single_backslash = r#"\"#;
    if text.ends_with(single_backslash) {
        return store_and_do_nothing(print_buffer, text);
    }
    if !print_buffer.is_empty() {
        return join_and_clear(print_buffer, text);
    }
    text.to_string()
}

fn post_process(print_buffer: &mut Vec<String>, text: &str) -> String {
    fix_newlines(print_buffer, text)
}

pub async fn request(
    prompt: String,
    _count: i64,
) -> TokioResult<Vec<ChatCompletionResponseStreamMessage>> {
    let mut print_buffer: Vec<String> = Vec::new();
    let config = &*CONFIGURATION.to_owned();
    let oconfig: OpenAIConfig = config.into();
    let openai = Client::with_config(oconfig);
    let completions = openai.chat();
    let messages = {
        CONVERSATION
            .lock()
            .await
            .push(string_to_chat_completion_request_user_message(
                prompt.clone(),
            ));
        CONVERSATION
            .lock()
            .await
            .clone()
            .into_iter()
            .collect::<Vec<_>>()
    };
    let mut request: CreateChatCompletionRequestArgs = config.into();
    let mut stream = completions
        .create_stream(request.messages(messages).build()?)
        .await?;
    IS_RUNNING.store(true, Ordering::SeqCst);

    let got_first_success: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let mut ret = vec![];

    'abort: while !ABORT.load(Ordering::Relaxed) {
        while let Some(c) = stream.next().await {
            match c {
                Ok(completion) => {
                    let completion = Arc::new(completion);
                    ret.push(completion.clone());
                    if !got_first_success.load(Ordering::SeqCst) {
                        got_first_success.store(true, Ordering::SeqCst);
                        print_response_prompt();
                    }
                    for choice in &completion.choices {
                        if ABORT.load(Ordering::Relaxed) {
                            break 'abort;
                        }
                        match choice.delta.content {
                            Some(ref text) => {
                                let newline_fixed = post_process(&mut print_buffer, &text);
                                print_and_flush(&newline_fixed);
                            }
                            None => {}
                        }
                        match choice.finish_reason {
                            Some(FinishReason::Stop) => {
                                debug!("Got stop from API, returning to REPL");
                                IS_RUNNING.store(false, Ordering::SeqCst);
                                break 'abort;
                            }
                            Some(reason) => {
                                let msg = format!("OpenAI API error: {reason:?}");
                                print_error(&msg);
                                continue 'abort;
                            }
                            None => {}
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("OpenAI API error: {e}");
                    print_error(&msg);
                    break 'abort;
                }
            }
        }
        debug!("Got end of stream, returning to REPL");
        IS_RUNNING.store(false, Ordering::SeqCst);
        break 'abort;
    }
    eprint_and_flush("\n");

    if !got_first_success.load(Ordering::SeqCst) {
        let msg = format!("Empty prompt, aborting.");
        print_error(&msg);
        return Ok(vec![]);
    }

    let result = ret
        .drain(..)
        .map(|o| Arc::new(o.choices.clone().into_iter().collect::<Vec<_>>()))
        .collect::<Vec<_>>()
        .drain(..)
        .map(|choice: Arc<Vec<ChatCompletionResponseStreamMessage>>| {
            choice
                .iter()
                .map(|choice| choice.clone())
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<Vec<_>>();

    let complete_message = result.iter().map(|o| o.delta.clone()).collect::<Vec<_>>();

    let assistant_msg = string_to_chat_completion_assistant_message(
        complete_message
            .into_iter()
            .map(|o| o.content.unwrap_or_else(String::new))
            .collect::<Vec<_>>()
            .join(""),
    );
    (*CONVERSATION).lock().await.push(assistant_msg);

    IS_RUNNING.store(false, Ordering::SeqCst);
    finish_prompt();
    Ok(result)
}
