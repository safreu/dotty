use std::{io};

use clap::Command;
use clap_complete::{Shell, generate};

pub fn execute(shell: Shell, mut command: Command) {
    generate(shell, &mut command, "dotty", &mut io::stdout());
    
}
