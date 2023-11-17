//! Help messages for the command-line interface.
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

use rustyline::Editor;

use crate::config;
use config::DEFAULT_CONFIG_FILENAME;
use std::fs::{self, File};
use std::io::Write as _;
use std::process::exit;

pub fn commands() {
    println!(include_str!("help/keybindings.txt"));
    exit(0);
}

const EXAMPLE_TOML: &str = r#"api_key = "<YOUR SECRET API KEY>"
model = "gpt-3.5-turbo"
max_tokens = 2048
temperature = 0.8"#;

pub fn missing_toml() {
    let default_path = config::default_path::<1>(None);
    eprintln!(
        r#"
Could not find the file `{1}`. To fix this, create {0}.

For example, use the following content (the text between the ```):

```
{EXAMPLE_TOML}
```

Here, replace `<YOUR SECRET API KEY>` with your API key, which you can request via https://beta.openai.com/account/api-keys.

The `max_tokens` sets the maximum amount of tokens that the server can answer with.
Longer answers will be truncated.

The `temperature` sets the `sampling temperature`. From the OpenAI API docs: "What sampling temperature to use. Higher values means the model will take more risks. Try 0.9 for more creative applications, and 0 (argmax sampling) for ones with a well-defined answer." According to Stephen Wolfram [1], setting it to a higher value such as 0.8 will likely work best in practice.


[1]: https://writings.stephenwolfram.com/2023/02/what-is-chatgpt-doing-and-why-does-it-work/

    "#,
        (&default_path).display(),
        DEFAULT_CONFIG_FILENAME.to_string_lossy()
    );
    let mut rl = Editor::<()>::new().unwrap();
    eprintln!(
        "Do you want me to write this example file to {0} for you to edit?",
        (&default_path).display()
    );
    let readline = rl.readline("[y/N] ");
    if let Ok(msg) = readline {
        if msg
            .trim()
            .chars()
            .nth(0)
            .map(|c| c.to_lowercase().collect::<String>() == "y")
            .unwrap_or(false)
        {
            if !default_path.exists() && !default_path.parent().unwrap().is_dir() {
                fs::create_dir_all(&default_path).expect("Could not make configuration directory");
            }
            let mut f = File::create(&default_path).expect("Unable to create file");
            f.write_all(EXAMPLE_TOML.as_bytes())
                .expect("Unable to write to file");
        }
    }
    exit(1);
}
