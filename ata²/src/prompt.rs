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
use atty;
use hyper::body::HttpBody;
use hyper::Body;
use hyper::Client;
use hyper::Method;
use hyper::Request;
use hyper_rustls::HttpsConnectorBuilder;
use log::debug;
use serde_json::json;
use serde_json::Value;

use std::error::Error;
use std::io::Write;
use std::result::Result;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub type TokioResult<T, E = Box<dyn Error + Send + Sync>> = Result<T, E>;

fn sanitize_input(input: String) -> String {
    let out = input.trim_end_matches("\n");
    out.replace('"', "\\\"")
}

fn print_and_flush(text: &str) {
    print!("{text}");
    std::io::stdout().flush().unwrap();
}

fn eprint_and_flush(text: &str) {
    eprint!("{text}");
    std::io::stderr().flush().unwrap();
}

pub fn eprint_bold(msg: &str) {
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
        eprint_bold("Prompt:\n");
    }
}

fn print_response_prompt() {
    if atty::is(atty::Stream::Stderr) {
        eprint_bold("Response:\n");
    }
}

fn finish_prompt(is_running: Arc<AtomicBool>) {
    is_running.store(false, Ordering::SeqCst);
    eprint_and_flush("\n\n");
    print_prompt();
}

pub fn print_error(is_running: Arc<AtomicBool>, msg: &str) {
    error!("{msg}");
    finish_prompt(is_running)
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

fn value2unquoted_text(value: &serde_json::Value) -> String {
    value.as_str().unwrap().to_string()
}

fn should_retry(line: &str, count: i64) -> bool {
    let v: Value = match serde_json::from_str(line) {
        Ok(line) => line,
        Err(_) => return false,
    };
    if v.get("error").is_some() {
        let error_type = value2unquoted_text(&v["error"]["type"]);
        let max_tries = 3;
        if count < max_tries && error_type == "server_error" {
            eprintln!(
                "\
                Server responded with a `server_error`. \
                Trying again... ({count}/{max_tries})\
                "
            );
            return true;
        }
    }
    false
}

#[tokio::main]
pub async fn request(
    abort: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    config: &super::Config,
    prompt: String,
    count: i64,
) -> TokioResult<bool> {
    is_running.store(true, Ordering::SeqCst);

    let api_key: String = config.clone().api_key;
    let model: String = config.clone().model;
    let max_tokens: i64 = config.clone().max_tokens;
    let temperature: f64 = config.temperature;

    let sanitized_input = sanitize_input(prompt.clone());
    let bearer = format!("Bearer {api_key}");
    // Passing newlines behind the prompt to get a more chat-like experience.
    let body = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": sanitized_input
            }
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": true
    })
    .to_string();

    let req = Request::builder()
        .method(Method::POST)
        .uri("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", bearer)
        .body(Body::from(body))?;

    let (mut req_parts, req_body) = req.into_parts();
    {
        req_parts
            .headers
            .get_mut("Authorization")
            .unwrap()
            .set_sensitive(true);
    }
    let req_body = &*(hyper::body::to_bytes(req_body).await?);
    let dbg_req_headers = format!("{:#?}", req_parts);
    let dbg_req_body = std::str::from_utf8(req_body).unwrap();
    let req = Request::from_parts(req_parts, Body::from(req_body.to_vec()));

    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_only()
        .enable_http1()
        .build();

    let client = Client::builder().build::<_, hyper::Body>(https);

    let mut response = match client.request(req).await {
        Ok(response) => response,
        Err(e) => {
            eprint_and_flush("\n");
            print_error(is_running, &e.to_string());
            return Ok(false);
        }
    };

    debug!(
        "Request:\n\nHeaders &c.:\n{}\n\nBody:\n{}",
        dbg_req_headers, dbg_req_body
    );

    // Do not move this in front of the request for UX reasons.
    eprint_and_flush("\n");

    let mut had_first_success = false;
    let mut data_buffer = vec![];
    let mut print_buffer: Vec<String> = vec![];
    while let Some(chunk) = response.body_mut().data().await {
        let chunk = chunk?;
        data_buffer.extend_from_slice(&chunk);

        let events = std::str::from_utf8(&data_buffer)?.split("\n\n");
        for line in events {
            if line.starts_with("data:") {
                let data: &str = &line[6..];
                if data == "[DONE]" {
                    finish_prompt(is_running);
                    return Ok(false);
                };
                let v: Value = serde_json::from_str(data)?;

                if v.get("choices").is_some() {
                    let delta = v.get("choices").unwrap()[0].get("delta");
                    if delta.is_none() {
                        // Ignoring wrong responses to avoid crashes.
                        continue;
                    }
                    if delta.unwrap().get("content").is_none() {
                        // Probably switching "role" (`"role":"assistant"`).
                        continue;
                    }
                    let text = value2unquoted_text(&delta.unwrap()["content"]);
                    let processed = post_process(&mut print_buffer, &text);
                    if !had_first_success {
                        had_first_success = true;
                        print_response_prompt();
                    };
                    print_and_flush(&processed);
                } else if v.get("error").is_some() {
                    let msg = value2unquoted_text(&v["error"]["message"]);
                    print_error(is_running, &msg);
                    return Ok(false);
                } else {
                    print_error(is_running, data);
                    return Ok(false);
                };
            } else if !line.is_empty() {
                if !had_first_success {
                    let retry = should_retry(line, count);
                    if retry {
                        return Ok(true);
                    } else {
                        print_error(is_running, line);
                        return Ok(false);
                    }
                };
                print_error(is_running, line);
                return Ok(false);
            };
            if abort.load(Ordering::SeqCst) {
                abort.store(false, Ordering::SeqCst);
                finish_prompt(is_running);
                return Ok(false);
            };
        }
        data_buffer.clear();
    }
    finish_prompt(is_running);
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_newlines() {
        assert_eq!(
            sanitize_input("foo\"bar".to_string()),
            "foo\\\"bar".to_string()
        );
    }

    #[test]
    fn value_is_unquoted() {
        use super::*;
        let v: Value = serde_json::from_str(r#"{"a": "1"}"#).unwrap();
        assert_eq!(value2unquoted_text(&v["a"]), "1");
    }
}
