# ratatui_tetris

A Tetris game for the terminal, built in Rust with [ratatui](https://ratatui.rs).

```
╭ HOLD ───────╮┏━━━━━━━━━━━━━━━━━━━━┓╭ NEXT ───────╮
│     []      │┃· · · [][][]· · · · ┃│       []    │
│   [][][]    │┃· · · · · · · · · · ┃│   [][][]    │
╰─────────────╯┃· · · · · · · · · · ┃╰─────────────╯
╭ STATS ──────╮┃· · · · · · · · · · ┃╭ LAST ───────╮
│SCORE    274 │┃· · · · · · · · · · ┃│ TETRIS      │
│LEVEL      1 │┃· · · · · · [][]· · ┃│ BACK-TO-BACK│
│LINES      0 │┃· · · · ::· · [][]· ┃│ 2 COMBO     │
│TIME    0:04 │┃· · []::::::· · [][]┃│ +1,200      │
│             │┃· · [][][]· · [][]· ┃│             │
│I          2 │┃· · · · [][][][]· · ┃│             │
│O          1 │┃· · · · [][][][][][]┃│             │
│T          2 │┃[][][]· · · · · [][]┃│             │
╰─────────────╯┗━━━━━━━━━━━━━━━━━━━━┛╰─────────────╯
```

`[]` is a locked or falling block, `::` is the ghost showing where the current
piece will land, and `·` marks empty cells.

## Running

```sh
cargo run --release
```

Needs a terminal at least 50x24. Anything smaller shows a size hint instead.

## Controls

| Key | Action |
| --- | --- |
| `←` `→` / `h` `l` | Move left / right |
| `↓` / `j` | Soft drop (1 point per row) |
| `space` | Hard drop (2 points per row) |
| `↑` / `x` / `k` | Rotate clockwise |
| `z` | Rotate counter-clockwise |
| `a` | Rotate 180° |
| `c` / `tab` | Hold the current piece |
| `p` / `esc` | Pause |
| `r` | Restart |
| `q` / `ctrl-c` | Quit |

## What's implemented

This follows the modern Tetris guideline rather than the 1984 original:

- **7-bag randomiser** — every permutation of the seven pieces is dealt before
  any piece repeats, so you never get a drought.
- **SRS rotation with wall kicks** — the full Super Rotation System kick tables
  for I and for J/L/S/T/Z, which is what makes T-spins possible.
- **Ghost piece** showing where the current piece will land.
- **Hold**, usable once per piece.
- **Next-piece preview**.
- **Lock delay** — 0.5s once grounded, refreshed by moving or rotating, capped
  at 15 refreshes so you can't stall forever.
- **T-spin detection**, including mini vs. full, using the three-corner rule.
- **Scoring**: singles through tetrises, T-spins, back-to-back bonuses, combos,
  perfect clears, and soft/hard drop points.
- **Levels** — one every 10 lines, with the guideline gravity curve
  (`(0.8 - 0.007·(level-1))^(level-1)` seconds per row).

## Control socket

The game can also be driven from outside the terminal:

```sh
cargo run --release -- --control /tmp/tetris.sock
```

It still renders and still takes the keyboard; this only adds a second way in.
A client sends one command per connection and gets the resulting state back as
one line of JSON, so a driver never has to scrape the screen.

| Command | Effect |
| --- | --- |
| `state` | Report the current state, changing nothing |
| `play <action> [action…]` | Apply actions in order, then report the state |
| `pause` / `resume` | Set the paused flag explicitly |

Action names are `left`, `right`, `soft`, `drop`, `cw`, `ccw`, `flip`, `hold` —
the same moves the keys map to. Anything unparseable comes back as
`{"error":"bad command"}`.

```sh
echo 'play cw left left drop' | nc -U /tmp/tetris.sock
```

```json
{
  "score": 20, "lines": 0, "level": 1,
  "phase": "falling", "can_hold": true, "hold": null,
  "next": ["Z"], "current": "T", "rotation": 0,
  "piece": [[4,-1],[3,0],[4,0],[5,0]],
  "ghost": [[4,14],[3,15],[4,15],[5,15]],
  "grid": ["..........", "...(18 more)...", "...I......"],
  "heights": [0,0,0,4,0,0,0,0,0,0],
  "holes": []
}
```

(Wrapped for reading; on the wire it is a single line.)

| Field | Meaning |
| --- | --- |
| `phase` | `falling`, `clearing`, `paused` or `gameover` |
| `next` | Upcoming piece letters, in order |
| `hold` / `can_hold` | Held piece, and whether hold is available this turn |
| `piece` / `ghost` | Four `[x, y]` cells; negative `y` is still above the well |
| `grid` | The **locked stack only** — 20 rows of 10, `.` for empty |
| `heights` | Per column, rows from the highest filled cell to the floor |
| `holes` | Every `[x, y]` empty cell buried under that column's surface |

The falling piece is reported separately from `grid` rather than stamped into
it, so a driver can't confuse the two. `heights` and `holes` are computed from
the real board, not inferred from what was drawn.

All game mutation stays on the main thread — the listener thread only passes
messages over a channel — so a client can't race the frame loop. The socket is
off unless `--control` is given, and it is Unix-only.

## Layout

| File | Responsibility |
| --- | --- |
| `src/tetromino.rs` | Piece shapes, colours, SRS kick tables, the 7-bag randomiser |
| `src/board.rs` | The playfield grid: collision, line clears, ghost projection |
| `src/control.rs` | The optional `--control` socket: commands, state JSON |
| `src/game.rs` | Rules: gravity, locking, hold, scoring, levels, T-spins |
| `src/ui.rs` | Rendering — playfield, panels, overlays |
| `src/main.rs` | Terminal setup, input mapping, the frame loop |

The game logic has no dependency on ratatui, and the rendering layer never
mutates game state, so both are tested independently.

## Tests

```sh
cargo test
```

50 tests covering rotation tables, every piece's spawn silhouette, bag
fairness, line-clear mechanics, scoring (including a canonical T-spin double
that verifies the kick tables place the piece in exactly the right cell),
lock-delay behaviour, input mapping, and rendering at a range of terminal
sizes.

To eyeball the layout without launching the game:

```sh
cargo test dump_layout -- --ignored --nocapture
```

## AI use

This is a vibe-coded project, but the main architectural design is mine.
