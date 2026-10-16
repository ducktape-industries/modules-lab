//! tetris-bot: a Tetris versus player with an account of its own.
//!
//! `tetris-bot --rpc <node url> --key <device.key> [--level easy|normal|hard|expert]`
//!
//! It holds one live session with the node at a time (`live.rs`) and plays
//! from it as any other side does (`versus.rs`, `ai.rs`): it waits in the
//! versus lobby, accepts every challenge, plays the round and asks for
//! another, and waits again when the other side leaves. The key is the
//! account's device key as the app keeps it in a file: the ed25519 seed,
//! 32 bytes. A lost session is opened again after a wait that doubles with
//! each failure, and one the node sent away (`GoAway`) after the wait it
//! named; a node that will not let the account play ends the process,
//! non-zero.
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;

use ai::Level;

mod ai;
mod live;
mod versus;

/// The longest wait before a session is opened again.
const MOST_WAIT: Duration = Duration::from_secs(60);
/// A session that lasted this long was healthy: the waits start over.
const HEALTHY: Duration = Duration::from_secs(60);

/// One line of the log, stamped with the UTC time of day.
pub fn log(line: impl std::fmt::Display) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (hours, minutes, seconds) = (now / 3600 % 24, now / 60 % 60, now % 60);
    println!("{hours:02}:{minutes:02}:{seconds:02} {line}");
}

fn main() -> ExitCode {
    let (rpc, key, level) = match args() {
        Ok(read) => read,
        Err(why) => {
            eprintln!(
                "tetris-bot: {why}\nusage: tetris-bot --rpc <node url> --key <device.key> [--level easy|normal|hard|expert]"
            );
            return ExitCode::from(2);
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    log(format!("level {}", level.name()));
    let why = runtime.block_on(serve(&rpc, &key, level));
    log(format!("stopped: {why}"));
    ExitCode::FAILURE
}

fn args() -> Result<(String, ed25519::PrivateKey, Level), String> {
    let mut args = std::env::args().skip(1);
    let (mut rpc, mut key, mut level) = (None, None, Level::Normal);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rpc" => rpc = args.next(),
            "--key" => key = args.next(),
            "--level" => {
                let name = args.next().unwrap_or_default();
                level = Level::ALL
                    .into_iter()
                    .find(|level| level.name() == name)
                    .ok_or(format!("no level {name}"))?;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let rpc = rpc.ok_or("no --rpc")?;
    let path = key.ok_or("no --key")?;
    let seed = std::fs::read(&path).map_err(|error| format!("{path}: {error}"))?;
    let key = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|_| format!("{path} holds no ed25519 key"))?;
    Ok((rpc, key, level))
}

/// Sessions one after another, until the node refuses the account.
async fn serve(rpc: &str, key: &ed25519::PrivateKey, level: Level) -> String {
    let mut failures = 0;
    loop {
        let opened = tokio::time::Instant::now();
        let why = match live::session(rpc, key, level).await {
            live::Stop::Refused(why) => return why,
            // not a failure: the node said when to come back
            live::Stop::GoAway(wait) => {
                log(format!(
                    "the node goes away; reconnecting in {} ms",
                    wait.as_millis()
                ));
                tokio::time::sleep(wait).await;
                continue;
            }
            live::Stop::Lost(why) => why,
        };
        if opened.elapsed() >= HEALTHY {
            failures = 0;
        }
        failures += 1;
        let wait = Duration::from_secs(1 << failures.min(6)).min(MOST_WAIT);
        log(format!(
            "session ended: {why}; reconnecting in {} s",
            wait.as_secs()
        ));
        tokio::time::sleep(wait).await;
    }
}
