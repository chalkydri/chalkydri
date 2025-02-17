use std::process::Command;

fn main() {
    Command::new("bun").args([
        "--cwd",
        "./ui",
        "build",
    ]).output().unwrap();
}
