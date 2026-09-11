# ADR 0006 — Fixed Logical Arena, Scaled Into a Resizable Window

- **Status:** Accepted.
- **Date:** 2026-09-11

## Context
In the browser the Arena was a fixed 960×600 logical space with a device-pixel-ratio transform on top, independent of the window size. A native application owns its window, so the playfield could instead be made to follow it.

## Decision
The Arena stays a fixed 960×600 logical space. The window is resizable and scales the view; the Arena never resizes. Every constant expressed in Arena units — spawn distance, Sensor Ray range, Asteroid counts, Episode caps — keeps its meaning, and a run depends only on its seed.

## Considered Options
- **The Arena follows the window** — rejected: difficulty would depend on window size, and the seed contract of ADR 0005 would break, since one seed would produce different runs at different window sizes.

## Consequences
The renderer scales a fixed logical space into a variable viewport, which is the same problem the HiDPI transform solved. Golden-frame tests can render at a fixed logical size and compare pixels across machines.
