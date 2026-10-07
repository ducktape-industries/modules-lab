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

## trade-view

Everything a spot terminal shows, on the program's rows:

- **The market's bar**: the market, which opens the list of markets with a
  search, the last price, how far it is from the price a day before, and
  the day's high, low and volume.
- **The chart** (`src/chart.rs`): candles by the bucket of time each one is,
  a bucket nothing traded in drawn flat at the close before it; volume, two
  moving averages, the last price and the reader's open orders on their
  lines; six spans, candles or a line. The wheel zooms, a drag pans, the
  arrow keys do both, and the page before the oldest candle held is asked
  for when the chart is panned back to it. The view draws it itself: one
  `canvas()` of quads and lines the host paints in order, the labels as
  elements laid over it.
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
   that point is on a candle, which shows the line the host paints there.
3. **Live, in the app.** The view is served with its program: pack
   `trade_view.wasm` and the describe module into `trade.wasm` with qa's
   `view-pack`, and found a stage whose `founding.toml` lists
   `[[programs]] id = "trade"`. To try a rebuilt view against a running
   stage, the app loads `<program>_view.wasm` out of `DUCKTAPE_VIEWS_DIR` in
   place of the view the network serves for that program:

   ```sh
   make wasm-views
   mkdir -p /tmp/views && cp target/wasm32-unknown-unknown/release/trade_view.wasm /tmp/views/
   DUCKTAPE_VIEWS_DIR=/tmp/views kit app NAME
   ```

   The file is read again each time the pane opens: rebuild, copy, close
   the pane and open it.

## Measured

trade-view has not been measured in the app yet. What is known of it is
native: the wide board (88 candles, the book, the tape, the ticket and three
open orders) is a 48 kB frame, `trade_view.wasm` is 1.1 MB, and a slot the
pointer enters sends 0.4 kB, the readout and the time under the candle, with
the canvas left as it was (`a_hover_sends_the_readout_and_none_of_the_chart`).

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

- **Where the pointer is on the chart.** A mouse event's position is in the
  window's pixels, and a view is not told where its elements sit in the
  window. So there is no horizontal line at the pointer, no price read off
  it and no zoom about it; the hovered candle is found by laying one element
  over each candle's slot, and its vertical line is that slot's hover style.
- **More than quads and lines on the canvas.** The builder has `rect`,
  `circle` and `line`. The wire carries paths, dashes, clips and transforms,
  which an area series, dashed order lines and an indicator pane need.
- **Text on the canvas.** Labels are elements positioned over it.
- **A field's face.** The host's one-line `Input` draws its text in the
  kit's own face and size: a view's `font_family` and `text_size` on the
  field are in the frame and do not reach the text (app
  `src/render/inputs.rs:308`). The ticket's price and amount are in the UI
  face, not in mono like the figures around them. `text_right` is kept.
- **A sensor's first size.** The app sends a sensor's first measurement to
  `on_show` only; `TestAppContext::simulate_measure` also sends it to a
  sensor that has only `on_resize`. A view that listens to `on_resize`
  alone passes its tests and draws nothing in the app.
- **gpui's names.** `canvas().rect` takes a `Bounds`, a share of a width
  is `relative`, a wrong field is `accesskit::Invalid`: the SDK re-exports
  none of them, so the view depends on gpui itself.
- **The time.** Without a clock the view does not know which candle is the
  current one: the chart ends at the newest candle the program holds.
- **Prices from outside.** A view asks programs and the host, nothing else.
  trade's prices are the ones its own orders trade at; real ones need
  something off the chain that places orders.
- **gpui-kit's charts.** `CandlestickChart`, the scales, axes, grid and
  tooltip in gpui-kit paint into the host's window, which a view does not
  hold. A view can only repeat their layout; reusing them means a chart node
  on the wire that the host draws.
