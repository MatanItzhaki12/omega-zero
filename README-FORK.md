# omega-zero, Slot edition: what this fork changes

This fork of [omega-zero](https://github.com/oauramos/omega-zero) 1.2.0 brings back the parts of
[Slot](https://github.com/BrandonKowalski/slot) that omega-zero left behind (its buttons, favorites,
recents, in-game menu and settings) and adds RetroAchievements. Everything is drawn in omega-zero's
own kit, so the new screens look like the rest of it. It is in daily use on an **Anbernic RG SP**.

The short version is in the [README](README.md#about-this-fork). This page covers each change in
full: what it does, its defaults, and the files it keeps on the card.

- [Controls](#controls)
- [Favorites and Recents](#favorites-and-recents)
- [Screen filter](#screen-filter)
- [Settings](#settings)
- [The in-game menu](#the-in-game-menu)
- [The POWER button](#the-power-button)
- [Labels and cart shells](#labels-and-cart-shells)
- [RetroAchievements](#retroachievements)
- [Files on the card](#files-on-the-card)
- [Building and installing](#building-and-installing)
- [Credits and licences](#credits-and-licences)

## Controls

The buttons are Slot's. The CONTROLS panel (a tap of SELECT on the shelf) lists them for the shelf
you are on.

### On the shelf

| Button | Does |
|---|---|
| A | Play the cart, picking up from its last state (see Load State on Insert) |
| Hold A | Play the cart from the start |
| ◀ ▶ | Browse |
| ▲ ▼ | Jump to the previous or next letter |
| L1 / R1 | Previous or next shelf |
| **Y** | Add the cart to Favorites, or take it off |
| **START** | Pick the core, on GBA and SNES carts (X still works too) |
| **MENU** | Settings |
| SELECT | CONTROLS |

### In a game

| Button | Does |
|---|---|
| **Tap MENU** | The in-game menu |
| Tap MENU twice | Save states |
| Hold MENU | Save a state and eject the cart |
| SELECT + MENU | Link play, or DISC on a PlayStation game on several discs |
| **SELECT + A / B** | Next or previous screen filter |
| SELECT + Y | Colour correction on or off |
| SELECT + L1 / R1 | Load the newest state / save a state |
| SELECT + ▲ ▼ / ◀ ▶ | Brightness / blue light |
| L2 | Rewind |
| R2 (twice to lock) | Fast forward |
| Hold X / Y | Turbo A / B |
| L1 / R1 | Stretch or restore the picture, on the Game Boys |
| Tap / hold POWER | Sleep / power off |

## Favorites and Recents

**Favorites.** Y on a cart adds it to a FAVORITES shelf, and every favorite gets a small gold
star at its top right corner, on every shelf it stands on. FAVORITES is sorted by system, in the
shelves' order (GBA, GB, GBC, SNES, PS1), then by name within each system. Taking the last star
off empties the shelf, and omega-zero moves on to the next one.

**Recents.** With Settings › Shelf › Recents on, a RECENTS shelf stands first: the last ten games
played, newest first. The boot opens on it.

Both lists are kept in Slot's own files and format (`Config/favorites.txt` and
`Config/recents.txt`, one `gba/<name>` per line), so a card moved over from Slot keeps them.

Both shelves draw each cart on the same face as its own system's shelf. A cart is drawn once, and a
new label shows on every shelf at the same time.

## Screen filter

A Game Boy, Game Boy Color or GBA game shown at 3x goes through one of three filters:

| Filter | Looks like |
|---|---|
| LCD | omega-zero's original LCD3x mask: scanlines and the panel's RGB stripes (default) |
| Grid | A thin dark line between the game's pixels, like Slot's Grid |
| Off | The game's pixels as they are |

Change it in Settings › CONFIGS › Screen Filter, in the in-game menu's Screen page, or with
SELECT + A / B in a game. A toast names the filter you switched to.

## Settings

MENU on the shelf opens Settings. Because the Settings window has room for only a few rows,
Slot's extra settings live in two panels that CONFIGS opens, like THEME.

**CONFIGS:** Fast Forward (OFF, 2X, 3X, 4X, 6X), Colour Correction, Screen Filter, Gameplay ›,
Shelf ›, Labels ›. GENERAL is unchanged.

**GAMEPLAY ›** (each row shows a line under the panel saying what it does)

| Row | Choices | Default |
|---|---|---|
| Fast Forward Sound | On, Off | Off |
| Rumble | On, Off | On |
| Load State on Insert | On, Off: off starts every game from its beginning | On |
| Save State on Eject | On, Off: the battery save is written either way | On |
| Rewind | On, Off | On |
| Turbo Buttons | On, Off | On |
| Frame Blending | Off, Mix, Ghosting (mGBA only; gpSP takes it as Mix) | Off |
| Game Boy Palette | Auto, then mGBA's 48 palettes (original Game Boy games on mGBA) | Auto |
| RetroAchievements | On, Off | Off |
| Account | Sign in on the keyboard (see [RetroAchievements](#retroachievements)) | |
| Hardcore | On, Off, from the next game | Off |

**SHELF ›**

| Row | Choices | Default |
|---|---|---|
| Recents | Show, Hide | Hide |
| Game Titles | Show, Hide: the picked cart's name under the top bar | Hide |
| Clock | 12-hour, 24-hour: the top bar and Date & Time | 24-hour |
| Lid Power Off | 1, 3, 5, 10, 15, 30 minutes | 3 |

Turbo and Rewind change at once, even in a game. Frame Blending and the Palette reach a running
game straight away.

## The in-game menu

A tap of MENU in a game opens a menu laid out like Slot's. The game pauses behind it, with the
top bar's clock and battery shown over it. The menu comes up 180 ms after the tap, so a double tap
goes straight to the save states without the menu flashing up first.

| Page | Rows |
|---|---|
| MENU | Resume · Undo Save · Undo Load · States · Settings › · Exit Game |
| SETTINGS | Screen › · Gameplay › · Controls › |
| SCREEN | Screen Filter · Palette · Colour Correction · Frame Blending · Brightness · Blue Light |
| GAMEPLAY | Fast Forward · Fast Forward Sound · Rewind · Turbo Buttons · Rumble · Load State on Insert · Save State on Eject |
| CONTROLS | Every button, for the system being played |

- **Undo Save** takes back the state just saved, and puts back the oldest state that the save
  pushed out of a full ring. **Undo Load** returns to where the game was before the load. Each
  is offered for 30 seconds.
- Rows a game has no use for are hidden, as on Slot: States with no states saved, Exit Game on a
  card with one cart, the Palette away from original Game Boy games.
- The SCREEN page does not dim the game, and its panel sits at the bottom, so each change shows
  on the game as you make it.
- B steps back one page, MENU closes the menu from any page, and holding MENU still ejects.

## The POWER button

Holding POWER powers off straight away, as on Slot. The game's state is saved first and the
"Turning off…" screen shows. The POWER menu, with its Restart, is gone. A tap of POWER still puts
the handheld to sleep.

## Labels and cart shells

- **`Labels/cart_shell.txt`.** omega-zero only read `Labels/cart_shell.ini`. It now also reads
  `cart_shell.txt`, the file Slot and [Cart Studio](https://studio.slot-cfw.fyi) write, in the
  same `Name = outline colour finish` format. When a game is in both files, the `.ini` wins.
- **Labels come in where you are looking.** At boot the labels of the shelf in view are drawn
  first, Favorites and Recents included. Moving to another shelf or cart moves the carts on
  screen to the front of the labels still waiting, so you never have to visit a cart's own
  system shelf to see its new label.
- **Mixed shelves draw cleanly.** Carts on screen are spaced by the widest of them, and the
  picked cart is drawn in front. A narrow cart beside a wide one, such as a Game Boy pak beside
  a GBA cart on Favorites, no longer covers it.

## RetroAchievements

Game Boy, Game Boy Color, GBA and SNES games are checked for
[RetroAchievements](https://retroachievements.org) as they play. This uses
[rcheevos](https://github.com/RetroAchievements/rcheevos) 12.5, the site's own client library, so
achievements are evaluated exactly as in RetroArch.

### Signing in

1. Settings › Gameplay › **Account**, then press A.
2. Type your user name, press START, then type your password and press START.
3. RetroAchievements turns on and signs in. The note under the panel shows *Signed in as …*, or
   the reason it could not sign in.

On the first successful sign-in omega-zero swaps the password for the login token the site
returns, as RetroArch does, so the password is not kept on the card. Y on Account signs out and
deletes the saved account.

You can also write `Config/retroachievements.ini` by hand:

```ini
username = YourName
password = YourPassword
```

The handheld needs Wi-Fi. Requests go out through `curl`, like the label downloads do.

### Playing

- When a game starts, a toast shows your progress, e.g. *3 of 52 unlocked*.
- Each achievement you earn pops up with its title. Mastering a game, leaderboard scores, and
  unlocks held back while offline (and sent later) get toasts too.
- **Hardcore** loads no states, rewinds nothing, and starts every game from its beginning rather
  than its last state, as the site asks. Fast forward still works. It applies from the next game.
- Without hardcore, loading a state or rewinding restarts achievement tracking, so the jump in
  the game's memory cannot earn anything.

### How it works

- `crates/omega-cheevos` builds rcheevos and a small C shim (`c/omega_cheevos.c`) and wraps them
  for Rust. Every call into the client is made under one lock. The emulator thread runs the
  per-frame check, and the main thread handles signing in and network answers.
- omega-retro now keeps the memory map a core declares (`RETRO_ENVIRONMENT_SET_MEMORY_MAPS`).
  rcheevos' `rc_libretro` reads the game's memory through it, which the GBA needs for both of its
  work RAMs.
- Games are identified by hashing the ROM file. The disc, zip and encrypted-ROM hashers are left
  out of the build.

### Not yet

- PlayStation games: their discs are not hashed.
- A list of a game's achievements: for now there are only toasts.

## Files on the card

| File | Holds |
|---|---|
| `Config/favorites.txt` | Favorites, one `gba/<name>` per line (Slot's format) |
| `Config/recents.txt` | The last ten games played, newest first (Slot's format) |
| `Config/retroachievements.ini` | The RetroAchievements user name, and the password until it is swapped for a token |
| `Labels/cart_shell.txt` | Cart shells as Slot and Cart Studio write them |

New keys in `Config/omega-zero.state`. A card from before this fork reads each one as its default.

| Key | Values |
|---|---|
| `screen_filter` | `lcd`, `grid`, `off` |
| `auto_load`, `eject_save`, `rewind`, `turbo` | `1`, `0` |
| `blending` | `off`, `mix`, `ghosting` |
| `gb_palette` | `auto`, or a palette's name as mGBA spells it |
| `show_recents`, `show_titles`, `clock_24h` | `1`, `0` |
| `lid_off_min` | `1`, `3`, `5`, `10`, `15`, `30` |
| `cheevos`, `cheevos_hardcore` | `1`, `0` |

`ff_speed` also takes `0` now, for OFF.

## Building and installing

This fork does not publish binaries yet. Build `omega-zero` for the H700 and copy it over the one
on a card that already runs omega-zero 1.2.0. The cores and scripts are unchanged.

The upstream route works as before and builds in an arm64 container:

```sh
task build:device
```

Or cross-compile without Docker, with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild),
which also cross-compiles rcheevos' C:

```sh
cargo zigbuild --profile device --target aarch64-unknown-linux-gnu.2.28 \
  -p omega-zero --no-default-features --features device
```

The binary is at `target/aarch64-unknown-linux-gnu/device/omega-zero`. Copy it to
`System/omega-zero` on the card, and keep the old one beside it (for example as
`omega-zero.1.2.0-original`) so you can go back.

### Tests

The new behaviour is covered by `crates/omega-zero/tests/suite/slot_controls.rs`, the unit tests in
`favorites.rs`, `achievements.rs` and `omega-cheevos`, and the updated settings, toast and
state tests. Tests that only showed the POWER menu are kept but ignored. So is the comparison of
Settings against `flow.html`'s screenshots, which show the CONFIGS rows from before this fork.

## Credits and licences

Everything credited in the [README](README.md#credits-and-licence) still applies. In addition:

- The controls, settings and in-game menu follow [Slot](https://github.com/BrandonKowalski/slot) by
  Brandon T. Kowalski (GPL-3.0). The Game Boy palette list and the frame blending options are
  ported from it.
- [rcheevos](https://github.com/RetroAchievements/rcheevos) by RetroAchievements.org (MIT),
  vendored in `crates/omega-cheevos/rcheevos`.
- `libretro.h` from [libretro-common](https://github.com/libretro/libretro-common) (MIT), in
  `crates/omega-cheevos/c/include`.

The fork is licensed, like omega-zero, under the GPL-3.0-or-later. It was written with Claude.
