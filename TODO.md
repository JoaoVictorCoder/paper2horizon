# TODO

## Next

- [ ] Translate simple method bodies, not only list their calls: a straight line of mapped calls
      (`event.getPlayer().sendMessage("...")` → `e.player.send_message("...")`), from the bytecode's stack.
- [ ] Read the delay and period a scheduler call passes (`runTaskTimer(this, task, 20L, 1200L)`) instead of a TODO.
- [ ] The main class's fields, as fields of the plugin's struct (with Rust types for the common Java ones).
- [ ] Follow calls into the plugin's own helpers (a listener calling `Util.send(player, ...)`) and list their API
      calls under the listener.
- [ ] Commands registered through Paper's Brigadier lifecycle events: their names, from the string literals.
- [ ] Java sources (`src/main/java`) as input, for plugins whose source is at hand: comments, names and generics.

## Tables

- [ ] More events and calls as horizon-plugin-api grows (chunk events, entities by handle, block entity data).
- [ ] Popular dependencies: Vault, PlaceholderAPI, ProtocolLib, WorldGuard: what to do instead of each.

## Repository

- [ ] CI: tests and clippy, and a weekly run on a set of real plugins to catch tables falling behind.
- [ ] Releases with built binaries for Linux, Windows and macOS.
