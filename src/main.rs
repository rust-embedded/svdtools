mod cli;
mod interrupts;

use cli::CliArgs;
use cli::Command;

fn main() {
    let args = CliArgs::get_arguments();
    match args.command {
        Command::Interrupts { svd_file, no_gaps } => {
            interrupts::parse_device(svd_file, !no_gaps);
        }
        _ => todo!(),
    }
}
