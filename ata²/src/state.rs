//! the global state of ata²
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

use clap::Parser as _;

use crate::args::Ata2;
use crate::config::{self, Config};
use crate::help;

use std::fs;
use std::fs::File;
use std::io::Read as _;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

lazy_static! {
    pub static ref FLAGS: Ata2 = Ata2::parse();
    pub static ref EXIT: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    pub static ref CONFIGURATION: Arc<Config> = {
        if FLAGS.print_shortcuts {
            help::commands();
            EXIT.store(true, Ordering::Relaxed);
        }
        let filename = FLAGS.config.location();
        if !filename.exists() {
            let v1_filename = FLAGS.config.location_v1();
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

        let config_ = Arc::new(Config::from(&contents));
        config_
    };
    pub static ref ABORT: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    pub static ref IS_RUNNING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    pub static ref HAD_FIRST_INTERRUPT: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}
