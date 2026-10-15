//! The client against a fake node on the other end of an in-process
//! websocket: the node's half of the handshake checks the proof as the
//! node does (`KeyScheme::verify`), then speaks the room and a match.
use arcade_view::tetris::Attack;
use arcade_view::versus::{Said, ask_track};
use commonware_codec::DecodeExt as _;
use live_wire::{Challenge, End, Mode};
use tokio::io::DuplexStream;
use tokio_tungstenite::tungstenite::protocol::Role;

use super::*;

const CHAIN: [u8; 32] = [5; 32];
/// The node's key, as `/v1/status` names it.
const NODE: [u8; 32] = [3; 32];
const NAMESPACE: &str = "lab#05050505/arcade/versus";
const ME: u64 = 13;

type Socket = WebSocketStream<DuplexStream>;

async fn linked() -> (Socket, Socket) {
    let (bot, node) = tokio::io::duplex(1 << 16);
    (
        WebSocketStream::from_raw_socket(bot, Role::Client, None).await,
        WebSocketStream::from_raw_socket(node, Role::Server, None).await,
    )
}

fn key() -> ed25519::PrivateKey {
    ed25519::PrivateKey::decode([7u8; 32].as_slice()).unwrap()
}

/// The node's next frame, pings and pongs passed over. What the bot owes
/// comes within a keep-alive: it answers at once.
async fn frame(node: &mut Socket) -> WsFrame {
    let next = async {
        loop {
            match node.next().await {
                Some(Ok(Message::Binary(bytes))) => return borsh::from_slice(&bytes).unwrap(),
                Some(Ok(_)) => continue,
                other => panic!("the bot's socket ended: {other:?}"),
            }
        }
    };
    tokio::time::timeout(KEEP_ALIVE, next)
        .await
        .expect("the bot said nothing")
}

/// The node's next control message, the boards between passed over.
async fn control(node: &mut Socket) -> Control {
    loop {
        match frame(node).await {
            WsFrame::Control(control) => return control,
            WsFrame::Datagram { .. } => continue,
            other => panic!("a control message, not {other:?}"),
        }
    }
}

async fn tell(node: &mut Socket, control: Control) {
    send(node, &WsFrame::Control(control)).await.unwrap();
}

/// The node's side of the handshake: the proof must verify.
async fn welcome(node: &mut Socket) {
    let Control::Hello(hello) = control(node).await else {
        panic!("a hello first");
    };
    assert_eq!((hello.chain, hello.scheme), (CHAIN, KeyScheme::Ed25519));
    let nonce = [9; 32];
    tell(node, Control::Challenge(Challenge { nonce })).await;
    let Control::Proof(proof) = control(node).await else {
        panic!("a proof");
    };
    let preimage = live_wire::proof_preimage(&CHAIN, &NODE, &nonce, &hello.key);
    assert!(KeyScheme::Ed25519.verify(
        &hello.key,
        live_wire::NAMESPACE,
        &preimage,
        &proof.signature
    ));
    let welcome = Welcome {
        account: ME,
        datagrams: false,
    };
    tell(node, Control::Welcome(welcome)).await;
}

#[test]
fn the_room_is_named_on_the_chain_as_the_app_names_it() {
    let status = |network: &str| Status {
        network: network.into(),
        time: 0,
        block_time_ms: 1000,
        epoch_length: 64,
        height: 1,
        tip: [0; 32],
        root: core_abi::Root([0; 32]),
        epoch: 0,
        identity: Vec::new(),
        contract: 5,
        genesis: [
            0xab, 0x01, 0xff, 0x10, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
            7, 7, 7, 7, 7, 7, 7,
        ],
        roles: core_abi::Roles {
            programs: "programs".into(),
            validators: "valset".into(),
            identity: "identity".into(),
            blobs: "blobs".into(),
            awaits: "awaits".into(),
            egress: None,
            halo: "halo".into(),
        },
    };
    let garage = status("garage");
    let read: Status = borsh::from_slice(&borsh::to_vec(&garage).unwrap()).unwrap();
    assert_eq!(namespace(&read.chain()), "garage#ab01ff10/arcade/versus");
    // a name that is no chain label stands alone, as `ChainId::of` leaves it
    assert_eq!(status("Lab_1").chain(), "Lab_1");
}

#[test]
fn a_group_heard_in_slices_numbers_its_objects_on() {
    let mut bot = Bot::new(ME, Level::Normal);
    let mut wire = Wire::new(NAMESPACE.into());
    bot.announced(ask_track(9, ME, "abc"), 9, true);
    for out in bot.out.drain(..) {
        wire.out(out);
    }
    let request = wire
        .requests
        .iter()
        .find_map(|(request, kind)| matches!(kind, Kind::Subscribe(_)).then_some(*request))
        .expect("their ask subscribed");
    let subscribed = Control::Subscribed {
        request,
        alias: 7,
        by: 9,
        mode: Mode::Stream,
        latest: None,
    };
    wire.heard(WsFrame::Control(subscribed), &mut bot).unwrap();
    // group 0 in two slices: an attack, then their top-out, object 1
    let slice = |said: Said| WsFrame::Group {
        header: GroupHeader { alias: 7, group: 0 },
        objects: vec![borsh::to_vec(&said).unwrap()],
        end: false,
    };
    let attack = Said::Attack(Attack { lines: 2, hole: 1 });
    wire.heard(slice(attack), &mut bot).unwrap();
    wire.heard(slice(Said::Out), &mut bot).unwrap();
    assert!(
        bot.log
            .iter()
            .any(|line| line.contains("against account 9: won")),
        "{:?}",
        bot.log
    );
}

#[tokio::test]
async fn the_handshake_signs_the_nonce_and_a_refusal_is_the_nodes_word() {
    let key = key();
    let (mut bot, mut node) = linked().await;
    let (welcomed, ()) = tokio::join!(handshake(&mut bot, CHAIN, &NODE, &key), welcome(&mut node));
    assert_eq!(welcomed.unwrap().account, ME);

    let (mut bot, mut node) = linked().await;
    let refuse = async {
        let _hello = control(&mut node).await;
        tell(&mut node, Control::Challenge(Challenge { nonce: [1; 32] })).await;
        let _proof = control(&mut node).await;
        let code = Code::NoAccount;
        tell(&mut node, Control::Refused { request: 0, code }).await;
    };
    let (refused, ()) = tokio::join!(handshake(&mut bot, CHAIN, &NODE, &key), refuse);
    assert!(matches!(refused, Err(Stop::Refused(why)) if why.ends_with("no_account")));
}

#[tokio::test(start_paused = true)]
async fn a_challenge_from_the_room_is_a_match_over_the_wire() {
    let (bot, mut node) = linked().await;
    let session = tokio::spawn(run(bot, NAMESPACE.into(), ME, Level::Normal));
    let Control::Watch {
        request: room,
        namespace,
    } = control(&mut node).await
    else {
        panic!("the room watched first");
    };
    assert_eq!(namespace, NAMESPACE);
    let Control::Publish {
        request: waiting,
        track,
        mode,
        ..
    } = control(&mut node).await
    else {
        panic!("waiting");
    };
    assert_eq!((track.name.as_str(), mode), ("wait/13", Mode::Stream));
    tell(
        &mut node,
        Control::Published {
            request: waiting,
            alias: 1,
        },
    )
    .await;

    // account 9 challenges it
    let theirs = ask_track(9, ME, "abc");
    let track = Track {
        namespace: NAMESPACE.into(),
        name: theirs.clone(),
    };
    let up = Control::Announce {
        request: room,
        track,
        by: 9,
        mode: Mode::Stream,
        up: true,
    };
    tell(&mut node, up).await;
    let Control::Publish {
        request: asking,
        track,
        ..
    } = control(&mut node).await
    else {
        panic!("its ask");
    };
    assert!(track.name.starts_with("ask/13/9/"));
    let Control::Publish {
        request: showing,
        track,
        mode,
        ..
    } = control(&mut node).await
    else {
        panic!("its board");
    };
    assert_eq!(
        (track.name.starts_with("board/13/"), mode),
        (true, Mode::Datagram)
    );
    let Control::Subscribe {
        request: hearing,
        track,
        from,
    } = control(&mut node).await
    else {
        panic!("their ask heard");
    };
    assert_eq!((track.name, from), (theirs.clone(), Start::Latest));
    assert_eq!(
        control(&mut node).await,
        Control::Unpublish { request: waiting }
    );
    tell(
        &mut node,
        Control::Published {
            request: asking,
            alias: 2,
        },
    )
    .await;
    tell(
        &mut node,
        Control::Published {
            request: showing,
            alias: 3,
        },
    )
    .await;
    let subscribed = Control::Subscribed {
        request: hearing,
        alias: 7,
        by: 9,
        mode: Mode::Stream,
        latest: None,
    };
    tell(&mut node, subscribed).await;

    // its well, a datagram on its board's alias
    let WsFrame::Datagram { header, payload } = frame(&mut node).await else {
        panic!("its board");
    };
    assert_eq!((header.alias, header.group), (3, 0));
    assert_eq!(borsh::from_slice::<Vec<u8>>(&payload).unwrap().len(), 200);

    // they top out: it won, and asks for another round on its ask
    let out = WsFrame::Group {
        header: GroupHeader { alias: 7, group: 0 },
        objects: vec![borsh::to_vec(&Said::Out).unwrap()],
        end: false,
    };
    send(&mut node, &out).await.unwrap();
    let rematch = loop {
        match frame(&mut node).await {
            WsFrame::Group {
                header,
                objects,
                end,
            } => break (header, objects, end),
            WsFrame::Datagram { .. } => continue,
            other => panic!("its rematch, not {other:?}"),
        }
    };
    assert_eq!((rematch.0.alias, rematch.0.group, rematch.2), (2, 0, false));
    assert_eq!(rematch.1, [borsh::to_vec(&Said::Rematch).unwrap()]);

    // they leave: it waits again
    let gone = Control::Ended {
        request: hearing,
        reason: End::PublisherGone,
    };
    tell(&mut node, gone).await;
    assert_eq!(
        control(&mut node).await,
        Control::Unpublish { request: asking }
    );
    assert_eq!(
        control(&mut node).await,
        Control::Unpublish { request: showing }
    );
    let Control::Publish { request, track, .. } = control(&mut node).await else {
        panic!("waiting again");
    };
    assert_eq!(track.name, "wait/13");

    // a room not open yet: the node holds its wait, and it waits with it
    let code = Code::NotGranted;
    tell(&mut node, Control::Waiting { request, code }).await;
    tell(&mut node, Control::Published { request, alias: 4 }).await;
    // the room's grant taken back ends the room's list, and the session
    let revoked = Control::Ended {
        request: room,
        reason: End::GrantRevoked,
    };
    tell(&mut node, revoked).await;
    assert!(matches!(session.await.unwrap(), Stop::Lost(why) if why.ends_with("GrantRevoked")));
}

#[tokio::test(start_paused = true)]
async fn the_socket_is_kept_alive_and_a_silent_one_ends() {
    let (bot, mut node) = linked().await;
    let session = tokio::spawn(run(bot, NAMESPACE.into(), ME, Level::Normal));
    // the node reads, and reading answers each ping
    let mut pings = 0;
    while pings < 4 {
        let heard = tokio::time::timeout(KEEP_ALIVE * 2, node.next()).await;
        match heard {
            Ok(Some(Ok(Message::Ping(_)))) => pings += 1,
            Ok(Some(Ok(Message::Binary(_)))) => {}
            other => panic!("a ping, not {other:?}"),
        }
    }
    assert!(!session.is_finished());
    // the node stops reading: its last pong left a ping ago
    tokio::time::sleep(IDLE).await;
    assert!(session.is_finished());
    assert_eq!(
        session.await.unwrap(),
        Stop::Lost("the live socket went quiet".into())
    );
}
