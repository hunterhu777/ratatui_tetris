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

## Layout

| File | Responsibility |
| --- | --- |
| `src/tetromino.rs` | Piece shapes, colours, SRS kick tables, the 7-bag randomiser |
| `src/board.rs` | The playfield grid: collision, line clears, ghost projection |
| `src/game.rs` | Rules: gravity, locking, hold, scoring, levels, T-spins |
| `src/ui.rs` | Rendering — playfield, panels, overlays |
| `src/main.rs` | Terminal setup, input mapping, the frame loop |

The game logic has no dependency on ratatui, and the rendering layer never
mutates game state, so both are tested independently.

## Tests

```sh
cargo test
```

48 tests covering rotation tables, bag fairness, line-clear mechanics, scoring
(including a canonical T-spin double that verifies the kick tables place the
piece in exactly the right cell), lock-delay behaviour, input mapping, and
rendering at a range of terminal sizes.

To eyeball the layout without launching the game:

```sh
cargo test dump_layout -- --ignored --nocapture
```
