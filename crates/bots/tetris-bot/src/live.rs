//! The live client: one session with the node over its websocket
//! (`live_wire::ROUTE`), as the app's is when the node's UDP is out of
//! reach. The node's `/v1/status` names the chain; the handshake is
//! `Hello` → `Challenge` → `Proof` → `Welcome`, the proof the device key's
//! signature under `live_wire::NAMESPACE`. Then the session watches the
//! versus room, and carries what the [`Bot`] asks and hears: control
//! messages, and `WsFrame` groups and datagrams both ways. The clocks are
//! the wire's: the socket's opening, the status and each handshake step
//! within [`HANDSHAKE_WAIT`]; a ping every [`KEEP_ALIVE`], since the node
//! closes a websocket silent for [`IDLE`]; and the node silent for `IDLE`
//! ends the session here.
use std::collections::HashMap;
use std::time::Duration;

use borsh::{BorshDeserialize, BorshSerialize};
use commonware_cryptography::{Signer as _, ed25519};
use ducklink::ChainId;
use futures_util::{SinkExt as _, StreamExt as _};
use live_wire::{
    Alias, Code, Control, DEFAULT_PRIORITY, DatagramHeader, Ending, GroupHeader, HANDSHAKE_WAIT,
    Hello, IDLE, KEEP_ALIVE, KeyScheme, Proof, RequestId, Start, Track, Welcome, WsFrame,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{Instant, MissedTickBehavior};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use crate::ai::Level;
use crate::versus::{Bot, Out};

/// Why a session ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// The path broke or the node went away: another session may open.
    Lost(String),
    /// The node is going away and said when to come back: another session
    /// opens after that wait.
    GoAway(Duration),
    /// The node will not let this account play: another session would be
    /// refused the same way.
    Refused(String),
}

/// What `/v1/status` answers: the node's own `Status`
/// (`noded::wire::Status`), field for field. Copied, not linked: the
/// node's crate carries the whole daemon. The tests decode what the node's
/// own type encodes, so a field the node adds fails them, not the bot.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Status {
    pub network: String,
    pub time: u64,
    pub block_time_ms: u64,
    pub epoch_length: u64,
    pub height: u64,
    pub tip: [u8; 32],
    pub root: core_abi::Root,
    pub epoch: u64,
    pub identity: Vec<u8>,
    pub contract: u32,
    pub genesis: [u8; 32],
    pub roles: core_abi::Roles,
}

impl Status {
    /// The chain's id, the first of a track's namespace, as the app
    /// stamps it: `ducklink::ChainId::of`, or the network's name alone
    /// where it is no chain label.
    pub fn chain(&self) -> String {
        ChainId::of(&self.network, &self.genesis)
            .map_or_else(|| self.network.clone(), |chain| chain.to_string())
    }
}

/// The versus room's namespace on `chain`: `<chain>/<program>/<room>`.
pub fn namespace(chain: &str) -> String {
    format!("{chain}/{}/{}", arcade::PROGRAM, arcade::VERSUS)
}

/// One session with the node at `rpc` (`http(s)://host[:port]`), as the
/// holder of `key`, to its end.
pub async fn session(rpc: &str, key: &ed25519::PrivateKey, level: Level) -> Stop {
    let rpc = rpc.trim_end_matches('/');
    let status = match status(rpc).await {
        Ok(status) => status,
        Err(why) => return Stop::Lost(format!("no status from {rpc}: {why}")),
    };
    let url = format!("{}{}", rpc.replacen("http", "ws", 1), live_wire::ROUTE);
    let mut socket = match tokio::time::timeout(
        HANDSHAKE_WAIT,
        tokio_tungstenite::connect_async(url.as_str()),
    )
    .await
    {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(error)) => return Stop::Lost(format!("no live socket at {url}: {error}")),
        Err(_) => return Stop::Lost(format!("the live socket at {url} did not open in time")),
    };
    let welcome = match tokio::time::timeout(
        HANDSHAKE_WAIT,
        handshake(&mut socket, status.genesis, &status.identity, key),
    )
    .await
    {
        Ok(Ok(welcome)) => welcome,
        Ok(Err(stop)) => return stop,
        Err(_) => return Stop::Lost("the node did not finish the handshake in time".into()),
    };
    let chain = status.chain();
    crate::log(format!(
        "session open on {chain} as account {}",
        welcome.account
    ));
    run(socket, namespace(&chain), welcome.account, level).await
}

async fn status(rpc: &str) -> Result<Status, String> {
    let client = reqwest::Client::builder()
        .timeout(HANDSHAKE_WAIT)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(format!("{rpc}/v1/status"))
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| error.to_string())?;
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    borsh::from_slice(&bytes).map_err(|error| format!("not a node's status: {error}"))
}

async fn send<S>(socket: &mut WebSocketStream<S>, frame: &WsFrame) -> Result<(), Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let bytes = borsh::to_vec(frame).expect("borsh encodes an in-memory value");
    socket
        .send(Message::Binary(bytes))
        .await
        .map_err(|error| Stop::Lost(format!("the live socket closed: {error}")))
}

/// The next control message, before the session runs.
async fn control<S>(socket: &mut WebSocketStream<S>) -> Result<Control, Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match socket.next().await {
            Some(Ok(Message::Binary(bytes))) => {
                return match borsh::from_slice(&bytes) {
                    Ok(WsFrame::Control(control)) => Ok(control),
                    _ => Err(Stop::Lost("the node sent no control message".into())),
                };
            }
            Some(Ok(_)) => continue,
            _ => return Err(Stop::Lost("the live socket closed".into())),
        }
    }
}

/// `Hello` → `Challenge` → `Proof` → `Welcome`, or the node's refusal. The proof names
/// `node`, the key `/v1/status` gave, so it is a proof to that node alone.
pub async fn handshake<S>(
    socket: &mut WebSocketStream<S>,
    chain: [u8; 32],
    node: &[u8],
    key: &ed25519::PrivateKey,
) -> Result<Welcome, Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let public = key.public_key().as_ref().to_vec();
    let hello = Hello {
        chain,
        scheme: KeyScheme::Ed25519,
        key: public.clone(),
    };
    send(socket, &WsFrame::Control(Control::Hello(hello))).await?;
    let Control::Challenge(challenge) = control(socket).await? else {
        return Err(Stop::Lost("the node did not challenge the hello".into()));
    };
    let preimage = live_wire::proof_preimage(&chain, node, &challenge.nonce, &public);
    let signature = key.sign(live_wire::NAMESPACE, &preimage).as_ref().to_vec();
    send(
        socket,
        &WsFrame::Control(Control::Proof(Proof { signature })),
    )
    .await?;
    match control(socket).await? {
        Control::Welcome(welcome) => Ok(welcome),
        Control::Refused { code, .. } => Err(Stop::Refused(format!(
            "the node refused this key a live session: {}",
            code.as_str()
        ))),
        Control::Hello(_)
        | Control::Challenge(_)
        | Control::Proof(_)
        | Control::Publish { .. }
        | Control::Unpublish { .. }
        | Control::Subscribe { .. }
        | Control::Unsubscribe { .. }
        | Control::Watch { .. }
        | Control::Unwatch { .. }
        | Control::Published { .. }
        | Control::Subscribed { .. }
        | Control::Subscribers { .. }
        | Control::Watching { .. }
        | Control::Announce { .. }
        | Control::Ended { .. }
        | Control::Waiting { .. }
        | Control::GoAway { .. } => Err(Stop::Lost("the node did not welcome the proof".into())),
    }
}

/// What a request of the session is.
#[derive(Clone, PartialEq, Eq)]
enum Kind {
    Watch,
    Publish(String),
    Subscribe(String),
}

/// A track this side publishes: its request, its alias once the node took
/// it, and what waits for the alias.
struct Publication {
    request: RequestId,
    alias: Option<Alias>,
    waiting: Vec<WsFrame>,
}

/// The session's requests, and the frames for the node.
struct Wire {
    namespace: String,
    next: RequestId,
    requests: HashMap<RequestId, Kind>,
    published: HashMap<String, Publication>,
    /// The subscription each alias in a data header names.
    aliases: HashMap<Alias, String>,
    /// The objects heard so far of each group a subscription is in: a
    /// group comes in slices, and an object's number is its place in it.
    heard: HashMap<(Alias, u64), u32>,
    frames: Vec<WsFrame>,
}

impl Wire {
    fn new(namespace: String) -> Wire {
        let mut wire = Wire {
            namespace,
            next: 1,
            requests: HashMap::new(),
            published: HashMap::new(),
            aliases: HashMap::new(),
            heard: HashMap::new(),
            frames: Vec::new(),
        };
        let request = wire.request(Kind::Watch);
        let namespace = wire.namespace.clone();
        wire.control(Control::Watch { request, namespace });
        wire
    }

    fn request(&mut self, kind: Kind) -> RequestId {
        let request = self.next;
        self.next += 1;
        self.requests.insert(request, kind);
        request
    }

    fn track(&self, name: String) -> Track {
        Track {
            namespace: self.namespace.clone(),
            name,
        }
    }

    fn control(&mut self, control: Control) {
        self.frames.push(WsFrame::Control(control));
    }

    /// `frame` of track `name` out once the node took the track. Of a
    /// datagram track only the newest waits.
    fn data(&mut self, name: &str, mut frame: WsFrame) {
        let Some(publication) = self.published.get_mut(name) else {
            return;
        };
        match publication.alias {
            Some(alias) => {
                aliased(&mut frame, alias);
                self.frames.push(frame);
            }
            None => {
                if matches!(frame, WsFrame::Datagram { .. }) {
                    publication
                        .waiting
                        .retain(|waiting| !matches!(waiting, WsFrame::Datagram { .. }));
                }
                publication.waiting.push(frame);
            }
        }
    }

    /// What the bot asked, as the wire says it.
    fn out(&mut self, out: Out) {
        match out {
            Out::Publish { name, mode } => {
                let request = self.request(Kind::Publish(name.clone()));
                let track = self.track(name.clone());
                self.published.insert(
                    name,
                    Publication {
                        request,
                        alias: None,
                        waiting: Vec::new(),
                    },
                );
                self.control(Control::Publish {
                    request,
                    track,
                    mode,
                    priority: DEFAULT_PRIORITY,
                });
            }
            Out::Unpublish { name } => {
                if let Some(publication) = self.published.remove(&name) {
                    self.requests.remove(&publication.request);
                    self.control(Control::Unpublish {
                        request: publication.request,
                    });
                }
            }
            Out::Subscribe { name } => {
                let request = self.request(Kind::Subscribe(name.clone()));
                let track = self.track(name);
                self.control(Control::Subscribe {
                    request,
                    track,
                    from: Start::Latest,
                });
            }
            Out::Send { name, group, body } => {
                self.data(&name, group_frame(group, vec![body], Ending::More))
            }
            Out::EndGroup { name, group } => {
                self.data(&name, group_frame(group, Vec::new(), Ending::Complete))
            }
            Out::Latest {
                name,
                group,
                object,
                body,
            } => {
                let header = DatagramHeader {
                    alias: 0,
                    group,
                    object,
                };
                self.data(
                    &name,
                    WsFrame::Datagram {
                        header,
                        payload: body,
                    },
                );
            }
        }
    }

    /// A frame from the node, to the bot.
    fn heard(&mut self, frame: WsFrame, bot: &mut Bot) -> Result<(), Stop> {
        let control = match frame {
            WsFrame::Control(control) => control,
            WsFrame::Group {
                header,
                objects,
                end,
            } => {
                let Some(name) = self.aliases.get(&header.alias) else {
                    return Ok(());
                };
                let key = (header.alias, header.group);
                let first = self.heard.get(&key).copied().unwrap_or(0);
                for (object, body) in (first..).zip(&objects) {
                    match borsh::from_slice(body) {
                        Ok(word) => bot.heard(name, object, word),
                        Err(error) => crate::log(format!("{name} said what no side says: {error}")),
                    }
                }
                // a group complete or cut has no more objects to number
                match end {
                    Ending::More => self.heard.insert(key, first + objects.len() as u32),
                    Ending::Complete | Ending::Cut => self.heard.remove(&key),
                };
                return Ok(());
            }
            // this side subscribes to no datagram track
            WsFrame::Datagram { .. } => return Ok(()),
        };
        match control {
            Control::Published { request, alias } => {
                let Some(Kind::Publish(name)) = self.requests.get(&request) else {
                    return Ok(());
                };
                let publication = self
                    .published
                    .get_mut(name)
                    .expect("a publication's request");
                publication.alias = Some(alias);
                for mut frame in std::mem::take(&mut publication.waiting) {
                    aliased(&mut frame, alias);
                    self.frames.push(frame);
                }
            }
            Control::Subscribed { request, alias, .. } => {
                if let Some(Kind::Subscribe(name)) = self.requests.get(&request) {
                    self.aliases.insert(alias, name.clone());
                    // a fresh opening: the node replays its group from the
                    // first object
                    self.heard.retain(|(heard, _), _| *heard != alias);
                }
            }
            // the watch is on; the room's tracks up now follow as `Announce`s
            Control::Watching { .. } => {}
            Control::Announce { track, by, up, .. } if track.namespace == self.namespace => {
                bot.announced(track.name, by, up)
            }
            Control::Ended { request, reason } => match self.requests.remove(&request) {
                Some(Kind::Subscribe(name)) => {
                    self.aliases.retain(|_, held| *held != name);
                    self.heard
                        .retain(|(alias, _), _| self.aliases.contains_key(alias));
                    bot.ended(&name);
                }
                Some(Kind::Publish(name)) => {
                    self.published.remove(&name);
                    crate::log(format!("the node ended {name}: {reason:?}"));
                }
                // the room's list ended: nothing is heard of the room
                Some(Kind::Watch) => {
                    return Err(Stop::Lost(format!(
                        "the node ended the session: {reason:?}"
                    )));
                }
                None => {}
            },
            // the room grants it nothing yet: the node holds the request and
            // answers it on the block that does, so the bot waits with it
            Control::Waiting { code, .. } => crate::log(format!(
                "{}: the versus room is not open yet; waiting for its grant",
                code.as_str()
            )),
            Control::Refused { request, code } => {
                match self.requests.get(&request).cloned() {
                    // an object dropped; the track goes on
                    Some(Kind::Publish(name)) if code == Code::TooLarge => crate::log(format!(
                        "the node dropped an object of {name}: {}",
                        code.as_str()
                    )),
                    Some(Kind::Subscribe(name)) => {
                        self.requests.remove(&request);
                        bot.ended(&name);
                    }
                    Some(_) => {
                        return Err(Stop::Lost(format!(
                            "the node refused a request: {}",
                            code.as_str()
                        )));
                    }
                    None => {}
                }
            }
            Control::GoAway { in_ms } => {
                return Err(Stop::GoAway(Duration::from_millis(u64::from(in_ms))));
            }
            Control::Hello(_)
            | Control::Challenge(_)
            | Control::Proof(_)
            | Control::Welcome(_)
            | Control::Publish { .. }
            | Control::Unpublish { .. }
            | Control::Subscribe { .. }
            | Control::Unsubscribe { .. }
            | Control::Watch { .. }
            | Control::Unwatch { .. }
            | Control::Subscribers { .. }
            | Control::Announce { .. } => {}
        }
        Ok(())
    }
}

fn group_frame(group: u64, objects: Vec<Vec<u8>>, end: Ending) -> WsFrame {
    let header = GroupHeader { alias: 0, group };
    WsFrame::Group {
        header,
        objects,
        end,
    }
}

fn aliased(frame: &mut WsFrame, alias: Alias) {
    match frame {
        WsFrame::Group { header, .. } => header.alias = alias,
        WsFrame::Datagram { header, .. } => header.alias = alias,
        WsFrame::Control(_) => {}
    }
}

/// The session as account `me`, the room `namespace` watched, to its end:
/// the round's clock ticks the [`Bot`], and what it asks goes out.
pub async fn run<S>(
    mut socket: WebSocketStream<S>,
    namespace: String,
    me: u64,
    level: Level,
) -> Stop
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut bot = Bot::new(me, level);
    let mut wire = Wire::new(namespace);
    let period = Duration::from_millis(arcade_view::tetris::PERIOD as u64);
    let mut clock = tokio::time::interval(period);
    clock.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut ping = tokio::time::interval_at(Instant::now() + KEEP_ALIVE, KEEP_ALIVE);
    let mut heard = Instant::now();
    loop {
        for line in bot.log.drain(..) {
            crate::log(line);
        }
        for out in bot.out.drain(..) {
            wire.out(out);
        }
        for frame in std::mem::take(&mut wire.frames) {
            if let Err(stop) = send(&mut socket, &frame).await {
                return stop;
            }
        }
        tokio::select! {
            _ = clock.tick() => bot.tick(),
            _ = ping.tick() => {
                if let Err(error) = socket.send(Message::Ping(Vec::new())).await {
                    return Stop::Lost(format!("the live socket closed: {error}"));
                }
            }
            _ = tokio::time::sleep_until(heard + IDLE) => {
                return Stop::Lost("the live socket went quiet".into());
            }
            message = socket.next() => {
                heard = Instant::now();
                let bytes = match message {
                    Some(Ok(Message::Binary(bytes))) => bytes,
                    Some(Ok(_)) => continue,
                    _ => return Stop::Lost("the live socket closed".into()),
                };
                let frame = match borsh::from_slice(&bytes) {
                    Ok(frame) => frame,
                    Err(error) => return Stop::Lost(format!("a frame off the wire: {error}")),
                };
                if let Err(stop) = wire.heard(frame, &mut bot) {
                    return stop;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
