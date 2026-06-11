mod cli;
mod document;
mod namespace;
mod utils;

fn main() {
    match cli::run() {
        Ok(output) => utils::output::write_success_and_exit(&output),
        Err(error) => utils::output::write_error_and_exit(&error),
    }
}
