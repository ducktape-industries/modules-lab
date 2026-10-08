# modules-lab

Experiments written against the ducktape modules SDK, out of tree. A crate
here keeps the shape of modules' `crates/app/*` (a view is a cdylib for
wasm32-unknown-unknown on `ducktape-view-guest`), so one that earns its place
moves over as it is.

| Path | What |
|---|---|
| `crates/app/trade` | spot markets for practice funds: the program, its types on the wire and its tables |
| `crates/app/trade-view` | its view: a trading terminal on the view SDK as it is |

The SDK is modules `dev` at the rev the app pins (`Cargo.toml`, `sdk-line`).
The app refuses a view built against another wire, so the pin moves when the
app's does.

## trade

An account takes its practice funds (`Op::Fund`), places limit and market
orders (`Op::Place`) and cancels them (`Op::Cancel`, `Op::CancelAll`); the
program matches them, best price first, then oldest first. Nothing runs
without an op, so a price moves only when an order trades. Three markets
(BTC/USD, ETH/USD, DUCK/USD), each with its own price tick; an amount counts
in lots of 0.0001. `src/lib.rs` holds the assets, the markets, the candle
spans and every type on the wire, `src/state.rs` the tables. A view reads
through `trade::ask::*`: each read names its reply and the tables it reads,
so a view asks it again only when a block wrote one of them.

A market's past is given at the founding: the program's params are a borsh
`Vec<History>`, a market's bars of one span, the last of them the stretch
before the founding block's. Each bar is written as its span's candle and
folded into every coarser span's; a finer span starts with none. A history
is candles, not trades: the tape and the fills start at the founding. With
no params every market starts empty. `examples/history.rs` writes a week of
every market's minutes, a seeded walk in integers, the same bytes on every
run:

```sh
cargo run -p trade --example history -- trade.params
```

## trade-view

Everything a spot terminal shows, on the program's rows:

- **The market's bar**: the market, which opens the list of markets with a
  search, the last price, how far it is from the price a day before, and
  the day's high, low and volume.
- **The chart** (`src/chart.rs`): candles by the bucket of time each one is,
  a bucket nothing traded in drawn flat at the close before it; volume, two
  moving averages, the last price and the reader's open orders on their
  lines; six spans, candles or a line. The wheel zooms about the pointer,
  from 40 px a candle down to one, 720 candles on screen at the most (five
  commands a candle, inside the 4096 one canvas carries): a week of 15m
  candles is one screen. A drag pans, the arrow keys do both (their zoom is
  about the right edge), and the pages before the oldest candle held are
  asked for as the chart is panned or zoomed back to them. The view draws
  it itself: one `canvas()` of quads and lines the host paints in order,
  the labels as elements laid over it. Its `sensor` tells where the host
  laid the chart out, so one listener on the plot turns the pointer's
  position into the candle under it: that candle's line, its time and the
  readout are three elements, and the frame holds none per candle.
- **The book and the tape** (`src/book.rs`): fifteen prices a side where
  they fit, each with what rests at it and the side's running total, in
  price steps of 1, 10 or 100 ticks. A press on a row puts its price in the
  ticket.
- **The ticket** (`src/ticket.rs`): buy or sell, at a limit or at the
  market, 25 to 100% of what the free funds afford, the total (estimated
  from the book on screen for a market order). An order off the tick, off
  the lot or over the free funds is not sent, and what the program refuses
  is said in place. A reader with no account reads everything and places
  nothing.
- **The panel** (`src/orders.rs`): open orders, every order, fills and
  balances. The two lists that go on are read a page at a time as they are
  scrolled.

From 1100 px of its own width the view is three columns; under that it is
the desk's pane, the chart over the panel and one column beside them with
the ticket, the book and the tape on tabs.

`src/data.rs` is everything the view asks: it follows the session and the
program's blocks, and a block reads again only what it wrote to
(`Change::touches`). After the reader's own op lands, everything is read
again. The view asks for no clock: the chart ends at the newest candle the
program holds, so a market nobody trades in is not drawn up to now.

## Loop

```sh
make test             # fmt, clippy, and each crate's tests: the view on a fake host
make wasm-programs    # each program for wasm32
make wasm-describes   # each program's describe module, ABI-checked
make wasm-views       # each view's wasm, optimized and ABI-checked by modules' own gate
make shots APP=/path/to/app   # the view's frames, painted by the app's real renderer
make live QA=/path/to/qa      # the module in the real app, on a stage of qa's kit ("A view on its own", 3)
```

`make wasm-views` needs Binaryen `wasm-opt` 132 and `wasm-tools`, as modules
does. `make shots` needs an app checkout at the rev that pins the same SDK
line, and a debug build of it (`APP_TARGET`, its target directory); the
app's `dev/screens/README.md` lists what the capture needs.

## A view on its own

There is no runner that keeps one view alive without a node. What there is:

1. **Its logic, natively.** `TestAppContext` opens the view on a fake host:
   a test answers the view's host methods, sends it clicks, typing, keys,
   wheel and hover, and reads the tree it draws (`src/tests/`). No window.
   The tests' market is `src/tests/sample.rs`: the rows of the design's
   boards, answered the way the program answers a query. The view holds no
   sample of its own.
2. **A frame, in the app's renderer.** `examples/frames.rs` drives the view
   to each screen of the boards on that same fake host and writes the frame
   it draws as JSON, and a debug build of the app paints it with
   `ducktape-app --render-tree <frame.json> --size WxH --theme dark`: the
   real renderer, theme and fonts, no node and no runtime. `make shots` does
   both and leaves `shots/*.png`. The frame is still: nothing answers a
   click. A fixture's `"hover": true` has the capture put the pointer at 60%
   of the width, 165 px over the bottom; `trade-hover` is a pane sized so
   that point is on a candle, and its frame holds the line the view draws
   on that candle.
3. **Live, in the app.** The view is served with its program, so it needs
   a network founded with `trade`: a stage of qa's `kit`. With a qa
   checkout, and `core`, `app` and `modules` checked out beside it as qa's
   README says:

   ```sh
   make live QA=/path/to/qa
   ```

   It builds the program, its describe module and the view, packs those
   two into `trade.wasm` with qa's `view-pack`, and founds stage `lab`
   (`STAGE=`) with it: kit's own founding with `live/founding.toml` after
   it, the week of history as the program's params. Then it seeds the
   stage with the ops `examples/seed.rs` writes. Wallet `dev`, the one the
   app opens with, gets an account and its funds; a second wallet, `bo`,
   rests 36 orders around BTC/USD's last price; two market orders of
   `dev`'s trade against them. It ends with the command that opens the app
   on the stage, a `kit app` with the lab's root: the wallet's password is
   `ducktape`, and Trade is on the rail. `live/live.sh` is all of it. Run
   again, it founds a fresh network on the same stage.

   The stage's node keeps running until `make live-down QA=/path/to/qa`
   stops it.

   The stage's modules is the rev this workspace pins, and its app is
   `APP_REF` (`dev` when not given), which must pin that same rev:
   `make live` stops and names both revs when it does not.

   Beyond the tools above it needs what qa's README asks of a machine that
   runs `kit`, and `git`, `python3` and `ps`. A first run builds kit, the
   node and the app.

   To try a rebuilt view against the running stage, the app loads
   `<program>_view.wasm` out of `DUCKTAPE_VIEWS_DIR` in place of the view
   the network serves for that program:

   ```sh
   make wasm-views
   mkdir -p /tmp/views && cp target/wasm32-unknown-unknown/release/trade_view.wasm /tmp/views/
   DUCKTAPE_VIEWS_DIR=/tmp/views kit app lab --root ...   # the command `make live` printed
   ```

   The file is read again each time the pane opens: rebuild, copy, close
   the pane and open it.

## Measured

trade-view in the app at 833487e8 with the host telling a sensor its
bounds, and the view built on that SDK, against the trade program on a kit
stage founded with the week of history (a release build, `DUCKTAPE_PERF=1`,
read at the test door's `/perf`, on a Ryzen 9 5950X). The view in a
1560 x 940 window, the chart 1048 px wide on 15m candles. One row is one
tick of the view: p50 of two runs, with p95 in brackets.

At the zoom the chart opens at: 123 candles, 8.5 px each, 515 nodes, 2 MB.

| What happened | fuel | wasm tick | decode | merge | replace | host render | frame |
|---|---|---|---|---|---|---|---|
| the pointer entering another candle's slot | 4.4M (4.5M) | 0.77 ms (1.0) | 0.11 ms (0.32) | 0.18 ms (0.29) | 0.19 ms (0.26) | 0.69 ms (0.82) | 0.42 kB |
| a wheel notch | 6.3M (7.2M) | 0.98 ms (1.1) | 0.19 ms (0.23) | 0.16 ms (0.18) | 0.18 ms (0.20) | 0.66 ms (0.71) | 27 kB (37 kB) |

Zoomed out to the whole week: 673 candles, 1.4 px each, 509 nodes, 3.7 MB.

| What happened | fuel | wasm tick | decode | merge | replace | host render | frame |
|---|---|---|---|---|---|---|---|
| the pointer entering another candle's slot | 6.3M (6.3M) | 1.0 ms (1.4) | 0.12 ms (0.26) | 0.29 ms (0.41) | 0.22 ms (0.28) | 0.71 ms (0.90) | 0.42 kB |
| a wheel notch | 16.7M (19.8M) | 2.1 ms (2.3) | 0.70 ms (0.84) | 0.21 ms (0.25) | 0.20 ms (0.22) | 0.66 ms (0.78) | 150 kB (190 kB) |

The frame holds no element for a candle, so a slot the pointer enters costs
about the same at 123 candles and at 673: 1.9 ms and 2.4 ms for the five
samples together. It sends the candle's line, the readout and the time
under it, and none of the canvas; a move inside one slot is a tick that
changes nothing, with no patch and no render. What still grows with the
candles is the view's own tick, which works the whole plot out and builds
every canvas command again on each render to find them unchanged, and a
wheel notch, which sends the canvas again. A row of the book under the
pointer is marked by the host with no tick; the line on a candle is the
view's, and moves on its tick.

These five samples stop at the element build: gpui's layout and paint of
the view are in none of them. Measured apart, with samples added to the app
for it, a slot the pointer entered at 673 candles took 27.8 ms when the
view laid an element over every candle, and 11.6 ms in a build of that view
without those elements. So read the tables as what the view's tick and the
host's work on the tree cost, not as the time from the pointer's move to
the pixels.

Before, the view found the candle under the pointer by laying one element
over each, whose hover style the host painted: the same week was 1,853
nodes and 8 MB, and a slot the pointer entered cost 29M fuel and 9.7 ms in
the five samples. A block that traded was measured then, on a market three
candles old: the view asks again what it wrote and draws for 5.7M fuel
(6.3M), a 0.74 ms tick (0.94) and a 0.35 kB frame (2.9 kB).

Natively the wide board (88 candles, the book, the tape, the ticket and
three open orders) is 665 nodes and a 41 kB frame, and a slot the pointer
enters sends 0.46 kB
(`a_hover_sends_the_line_and_the_readout_and_none_of_the_chart`).
`trade_view.wasm` is 1.1 MB.

The program on a node with core's README limits (1e9 fuel, 256 MB): the
founding took the week of every market's minutes (30,240 bars, 39 thousand
candles written), and an order that trades against 32 makers, the most one
order may, was applied.

The numbers below are the spike's, market-view, the chart alone on a made-up
series, in the app at 833487e8 (a release build, `DUCKTAPE_PERF=1`, read at
the test door's `/perf`), on a Ryzen 9 5950X. One redraw is one tick: a slot
the pointer enters, a wheel notch, a drag step, a clock tick. p50, with p95
in brackets:

| Candles on screen | fuel | wasm tick | decode + merge + replace | host render | frame |
|---|---|---|---|---|---|
| 120 | 4.9M | 0.75 ms (0.8) | 0.4 ms | 0.35 ms (0.4) | 23 kB |
| 240 | 9.3–9.6M | 1.1–1.3 ms (1.5) | 0.5–0.6 ms | 0.5–0.6 ms (0.7) | 45 kB |

That is a redraw that changes the chart. A hover does not. The mark on the
hovered candle is its slot's hover style, which the host paints as the
pointer moves, with no tick of the view. Only the readout takes a tick, when
the pointer enters another slot: the view renders, and 100 B crossed in
place of the frame.

| Candles on screen | fuel | wasm tick | decode + merge + replace | host render | frame |
|---|---|---|---|---|---|
| 120, a slot entered | 3.4M | 0.45 ms (0.6) | 0.2 ms | 0.3 ms (0.4) | 0.1 kB |
| 240, a slot entered | 6.3M | 0.75 ms (1.2) | 0.35 ms | 0.5 ms (0.75) | 0.1 kB |

The app's limit is 250M fuel a tick, and the spike's memory stayed under
2.7 MB. A redraw costs in proportion to what is on screen, since every one
sends the whole canvas again, and the readout's tick does too, since a view
renders its whole tree whatever changed. trade-view renders more than the
spike did on every tick: expect its numbers higher. What the GPU takes to
paint the quads is not in these numbers: the box that ran them paints in
software.

## What the SDK does not give a trading view yet

- **A mark that follows the pointer with no tick.** The view knows where
  the pointer is on its chart: the chart's `sensor` tells its bounds in the
  window's pixels (`on_bounds`), the pixels a mouse event's position is in,
  and one listener finds the candle under it. What it draws there it draws
  on its own tick: the line on the hovered candle takes a tick for each slot
  the pointer enters, where a hover style is the host's alone and takes
  none. A horizontal line at the pointer and the price read off it are not
  drawn yet; they wait for nothing of the SDK's.
- **More than quads and lines on the canvas.** The builder has `rect`,
  `circle` and `line`. The wire carries paths, dashes, clips and transforms,
  which an area series, dashed order lines and an indicator pane need.
- **Text on the canvas.** Labels are elements positioned over it.
- **A field's face.** The host's one-line `Input` draws its text in the
  kit's own face and size: a view's `font_family` and `text_size` on the
  field are in the frame and do not reach the text (app
  `src/render/inputs.rs:308`). The ticket's price and amount are in the UI
  face, not in mono like the figures around them. `text_right` is kept.
- **gpui's names.** `canvas().rect` takes a `Bounds` made of a `point` and
  a `size`, and a share of a width is `relative`: the SDK re-exports
  `Bounds` and none of the three functions, so the view depends on gpui
  itself.
- **The time.** Without a clock the view does not know which candle is the
  current one: the chart ends at the newest candle the program holds.
- **Prices from outside.** A view asks programs and the host, nothing else.
  trade's prices are the ones its own orders trade at; real ones need
  something off the chain that places orders.
- **gpui-kit's charts.** `CandlestickChart`, the scales, axes, grid and
  tooltip in gpui-kit paint into the host's window, which a view does not
  hold. A view can only repeat their layout; reusing them means a chart node
  on the wire that the host draws.
