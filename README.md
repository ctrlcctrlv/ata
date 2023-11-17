<h1 align="center"><code>ata²</code>: Ask the Terminal Anything</h1>

<h3 align="center">ChatGPT in the terminal</h3>

[![asciicast](https://asciinema.org/a/sOgAo4BkUXBJTSgyjIZw2mnFr.svg)](https://asciinema.org/a/sOgAo4BkUXBJTSgyjIZw2mnFr)

## This is a fork!

The original project, `ata`, by Rik Huijzer is [elsewhere](https://github.com/rikhuijzer/ata).

This fork implements many new config options and features.

<h3 align=center>
TIP:<br>
  Run a terminal with this tool in your background and show/hide it with a keypress.<br>
    This can be done via: Iterm2 (Mac), Guake (Ubuntu), scratchpad (i3/sway), yakuake (KDE), or the quake mode for the Windows Terminal.
</h3>

## Productivity benefits

- The terminal starts more quickly and requires **less resources** than a browser.
- The **keyboard shortcuts** allow for quick interaction with the query. For example, press `CTRL + c` to cancel the stream, `CTRL + ↑` to get the previous query again, and `CTRL + w` to remove the last word.
- A terminal can be set to **run in the background and show/hide with one keypress**. To do this, use iTerm2 (Mac), Guake (Ubuntu), scratchpad (i3/sway), or the quake mode for the Windows Terminal.
- The prompts are **reproducible** because each prompt is sent as a stand-alone prompt without history. Tweaking the prompt can be done by pressing `CTRL + ↑` and making changes.

## Usage

Download the binary for your system from [Releases](https://github.com/ctrlcctrlv/ata2/releases).
If you're running Arch Linux, then you can use the AUR package: [ata2](https://aur.archlinux.org/packages/ata2)

To specify the API key and some basic model settings, start the application.
It should give an error and the option to create a configuration file called `ata2.toml` for you.
Press `y` and `ENTER` to create a `ata2.toml` file.

Next, request an API key via <https://beta.openai.com/account/api-keys> and update the key in the example configuration file.

For more information, see:

```sh
$ ata2 --help
```

## FAQ

**How much will I have to pay for the API?**

Using OpenAI's API for chat is very cheap.
Let's say that an average response is about 500 tokens, so costs $0.001.
That means that if you do 100 requests per day, which is a lot, then that will cost you about $0.10 per day ($3 per month).
OpenAI grants you $18.00 for free, so you can use the API for about 180 days (6 months) before having to pay.

**How does this compare to LLM-based search engines such as You.com or Bing Chat?**

At the time of writing, the OpenAI API responds much quicker than the large language model-based search engines and contains no adds.
It is particularly useful to quickly look up some things like Unicode symbols, historical facts, or word meanings.

**Can I build the binary myself?**

Yes, you can clone the repository and build the project via [`Cargo`](https://github.com/rust-lang/cargo).
Make sure that you have `Cargo` installed and then run:

```sh
$ git clone https://github.com/ctrlcctrlv/ata2.git

$ cd ata2/

$ cargo build --release
```
After this, your binary should be available at `target/release/ata2` (Unix-based) or `target/release/ata2.exe` (Windows).

You may also:

```sh
$ cargo install --path .
```

# License

   Copyright 2023 Fredrick R. Brennan &lt;copypaste@kittens.ph&gt;, Rik Huijzer &lt;rikhuijzer@pm.me&gt;, &amp; ATA Project Authors

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.

