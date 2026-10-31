# TODO

## Next

What keeps bodies from translating, most common first (66 small plugins from Modrinth, 23 of 277 translated):

- [ ] Fields of listener and command classes other than the plugin reference (`HungerListener.lastFoodLevels`,
      `PlayerSitListener.seatManager`): fields of the plugin's struct. Now the most common blocker.
- [ ] Static fields and singletons (`Main.getInstance()`, `ConfigHandler.INSTANCE`): the plugin itself.
- [ ] `try { Integer.parseInt(s) } catch (NumberFormatException e)` as `match s.parse::<i32>()`.
- [ ] `||` in conditions, `?:`, `continue` and `break` in loops, index loops (`for (int i = 0; ...)`).
- [ ] Lambdas inside bodies (`runTaskLater(this, () -> ..., 20L)` in a listener): closures.
- [x] State in collections: fields typed `Map<UUID, Long>`, `Set<UUID>`, `List<String>` (from their generic
      `Signature`) as `HashMap`, `HashSet`, `Vec`; `new ArrayList<>()` in a body, typed by its local's signature.
- [x] `null`: `Option` for what can be missing (`Bukkit.getPlayer(name)`, `map.get(k)`); `if (x != null)` as
      `if let Some(x) = x`, `if (x == null) return` as `let Some(x) = ... else`.
- [ ] `Option` for the plugin's own fields that start `null`, and helpers that return `null`.
- [x] Translate method bodies: listeners, commands, completers, tasks, and the plugin's own methods they call, with
      `cargo check` taking back what doesn't compile.
- [x] Read the delay and period a scheduler call passes.
- [x] The main class's simple fields, as fields of the plugin's struct.
- [ ] Commands registered through Paper's Brigadier lifecycle events: their names, from the string literals.
- [ ] Java sources (`src/main/java`) as input, for plugins whose source is at hand: comments, names and generics.

## Tables

- [ ] More events and calls as horizon-plugin-api grows (chunk events, entities by handle, block entity data).
- [ ] Popular dependencies: Vault, PlaceholderAPI, ProtocolLib, WorldGuard: what to do instead of each.

## Repository

- [ ] CI: tests and clippy, and a weekly run on a set of real plugins to catch tables falling behind.
- [ ] Releases with built binaries for Linux, Windows and macOS.
