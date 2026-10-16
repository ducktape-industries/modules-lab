//! One program as a founding takes it: `cargo run -p trade --example pack
//! -- SECTION PROGRAM PAYLOAD OUT` writes PROGRAM with PAYLOAD in it as the
//! custom section SECTION, `ducktape.view` (a view), `ducktape.describe`
//! (a describe module) or `ducktape.notify` (a notifier), through
//! programs' own packer (`founding::embed`, `founding::embed_describe`,
//! `founding::embed_notify`), which checks each and replaces a section of
//! that name already there. OUT may be PROGRAM. Every program of a lab
//! stage goes through it (`live/live.sh`), not trade's alone.
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let [section, program, payload, out] = args.as_slice() else {
        eprintln!("usage: pack SECTION PROGRAM PAYLOAD OUT");
        process::exit(2);
    };
    let read = |path: &str| fs::read(path).unwrap_or_else(|error| die(path, error));
    let (code, payload) = (read(program), read(payload));
    let packed = match section.as_str() {
        founding::VIEW_SECTION => founding::embed(&code, &payload),
        founding::DESCRIBE_SECTION => founding::embed_describe(&code, &payload),
        founding::NOTIFY_SECTION => founding::embed_notify(&code, &payload),
        other => die(other, "no section a founding reads"),
    };
    let packed = packed.unwrap_or_else(|error| die(program, error));
    fs::write(out, packed).unwrap_or_else(|error| die(out, error));
}

fn die(what: &str, why: impl std::fmt::Display) -> ! {
    eprintln!("pack: {what}: {why}");
    process::exit(1);
}
