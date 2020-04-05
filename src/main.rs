mod cli;
mod common;
mod interrupts;
mod mmap;
mod patch;
mod makedeps;

fn main() {
    cli::run();
}
