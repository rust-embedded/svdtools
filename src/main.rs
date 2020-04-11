mod cli;
mod common;
mod interrupts;
mod makedeps;
mod mmap;
mod patch;

fn main() {
    cli::run();
}
