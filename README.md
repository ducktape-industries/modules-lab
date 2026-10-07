# modules-lab

Experiments written against the ducktape modules SDK, out of tree. A crate
here keeps the shape of modules' `crates/app/*` (a view is a cdylib for
wasm32-unknown-unknown on `ducktape-view-guest`), so one that earns its place
moves over as it is.

| Path | What |
|---|---|
| `crates/app/market-view` | a candlestick chart over a made-up market: can a trading view be written against the view SDK as it is |

The SDK is modules `dev` at the rev the app pins (`Cargo.toml`, `sdk-line`).
The app refuses a view built against another wire, so the pin moves when the
app's does.

## market-view

Candles, volume bars, a ruled price axis, a time axis, the last price on its
line, a symbol and a candle-span picker, a band on the candle under the
pointer and a readout of it. The wheel zooms, a drag pans, the arrow keys do
both, and a `clock.ticks` subscription moves the live candle once a second.

The view draws the chart itself: one `canvas()` of quads the host paints in
order (three a candle: wick, body, volume) and the labels as elements laid
over it. No program holds prices, so `src/series.rs` makes each symbol's
candles from a seed.

## Loop

```sh
make test         # fmt, clippy, and each view on a fake host (native)
make wasm-views   # the wasm, optimized and ABI-checked by modules' own gate
make shots APP=/path/to/app   # the frames, painted by the app's real renderer
```

`make wasm-views` needs Binaryen `wasm-opt` 132 and `wasm-tools`, as modules
does. `make shots` needs an app checkout at the rev that pins the same SDK
line; the app's `dev/screens/README.md` lists what the capture needs.

## A view on its own

There is no runner that keeps one view alive without a node. What there is:

1. **Its logic, natively.** `TestAppContext` opens the view on a fake host:
   a test answers the view's host methods, sends it clicks, keys, wheel and
   hover, and reads the tree it draws (`src/tests.rs`). No window.
2. **One frame, in the app's renderer.** `examples/frames.rs` writes the
   frame a view draws as JSON, and a debug build of the app paints it with
   `ducktape-app --render-tree <frame.json> --size WxH --theme dark`: the
   real renderer, theme and fonts, no node and no runtime. `make shots` does
   both and leaves `shots/*.png`. The frame is still: nothing answers a click.
3. **Live, in the app.** The app loads `<program>_view.wasm` out of
   `DUCKTAPE_VIEWS_DIR` in place of the view the network serves for that
   program, and takes the tab's name from the file's own manifest. It needs
   a node whose roster lists the program; qa's founding lists `explorer` as
   a view with no program behind it, so on a `kit` stage:

   ```sh
   make wasm-views
   mkdir -p /tmp/views && cp target/wasm32-unknown-unknown/release/market_view.wasm /tmp/views/explorer_view.wasm
   DUCKTAPE_VIEWS_DIR=/tmp/views kit app NAME    # the Explorer tab is now Market
   ```

   The file is read again each time the pane opens: rebuild, copy, close
   the pane and open it.

## Measured

market-view in the app at 833487e8 (a release build, `DUCKTAPE_PERF=1`,
read at the test door's `/perf`), on a Ryzen 9 5950X. One redraw is one tick:
a slot the pointer enters, a wheel notch, a drag step, a clock tick. p50,
with p95 in brackets:

| Candles on screen | fuel | wasm tick | decode + merge + replace | host render | frame |
|---|---|---|---|---|---|
| 120 | 4.9M | 0.75 ms (0.8) | 0.4 ms | 0.35 ms (0.4) | 23 kB |
| 240 | 9.3–9.6M | 1.1–1.3 ms (1.5) | 0.5–0.6 ms | 0.5–0.6 ms (0.7) | 45 kB |

That is a redraw that changes the chart: a clock tick, a drag step, a wheel
notch. A hover does not. The band on the hovered candle is its slot's hover
style, which the host paints as the pointer moves, with no tick of the
view. Only the readout takes a tick, when the pointer enters another slot:
the view renders, and 100 B cross in place of the frame.

| Candles on screen | fuel | wasm tick | decode + merge + replace | host render | frame |
|---|---|---|---|---|---|
| 120, a slot entered | 3.4M | 0.45 ms (0.6) | 0.2 ms | 0.3 ms (0.4) | 0.1 kB |
| 240, a slot entered | 6.3M | 0.75 ms (1.2) | 0.35 ms | 0.5 ms (0.75) | 0.1 kB |

The app's limit is 250M fuel a tick, and the guest's memory stays under
2.7 MB. A redraw costs in proportion to the candles on screen, since every
one sends the whole canvas again, and the readout's tick does too, since a
view renders its whole tree whatever changed. What the GPU takes to paint
the quads is not in these numbers: the box that ran them paints in software.

## What the SDK does not give a chart yet

- **Where the pointer is on the chart.** A mouse event's position is in the
  window's pixels, and a view is not told where its elements sit in the
  window. So there is no crosshair at the pointer and no zoom about it; the
  hovered candle is found by laying one element over each candle's slot.
- **More than quads on the canvas.** The builder has `rect`, `circle` and
  `line`. The wire carries paths, dashes, clips and transforms, which a line
  or area series, dashed rules and an indicator need.
- **Text on the canvas.** Labels are elements positioned over it.
- **A sensor's first size.** The app sends a sensor's first measurement to
  `on_show` only; `TestAppContext::simulate_measure` also sends it to a
  sensor that has only `on_resize`. A view that listens to `on_resize`
  alone passes its tests and draws nothing in the app.
- **gpui's geometry names.** `canvas().rect` takes a `Bounds`, which the
  SDK does not re-export, so a view that draws depends on gpui itself.
- **Prices.** A view asks programs and the host, nothing else: there is no
  door to the network. Real prices need a program that holds candles and
  something off the chain that writes them.
- **gpui-kit's charts.** `CandlestickChart`, the scales, axes, grid and
  tooltip in gpui-kit paint into the host's window, which a view does not
  hold. A view can only repeat their layout; reusing them means a chart node
  on the wire that the host draws.
