# paper2horizon

Turns a **Paper or Bukkit plugin `.jar`** into a Rust crate for
[Horizon Server Plugins](https://github.com/JoaoVictorCoder/horizon-server-plugins)' `horizon-plugin-api`, with the
plugin's shape already in place and what's left to port marked in it.

Paper plugins are Java bytecode written against Bukkit's classes, so they can't run on a Horizon server as they
are. paper2horizon reads the jar (no source code needed) and writes:

- **`plugin.yml`** as it was (Horizon reads Bukkit's), or one made from `paper-plugin.yml`;
- the plugin's **resources** (`config.yml`, `messages.yml`, `lang/`...);
- **`src/lib.rs`**: a listener for each `@EventHandler` on the matching Horizon event, with its priority and
  `ignoreCancelled`; an executor and a tab completer for each command; the tasks `onEnable` schedules, with their
  ticks; the main class's simple fields as the plugin's fields; `ctx.save_default_config` when the plugin saves
  its `config.yml`. **Method bodies are translated to Rust** where paper2horizon can (below); the others hold a
  `TODO` that lists what the Java code called, each call mapped to its horizon-plugin-api equivalent (or why there
  is none yet), the constants it read (`Material.BEDROCK` → `"minecraft:bedrock"`) and the text it used;
- **`PORTING.md`**: a summary, the listeners and commands, what the plugin needs that Horizon lacks, bundled
  libraries, and warnings (plugins that reach into Minecraft's own code or CraftBukkit, Brigadier commands,
  dependencies on other plugins).

The crate compiles as it is: the plugin loads, registers its commands and listens to its events. What was
translated works; each `TODO` does nothing until it is ported.

## Translating method bodies

The bytecode of each listener, command, tab completer and task is run symbolically, with typed expressions on its
stack, and comes back as Rust: calls through the same tables as the notes, `if`/`else` (and `&&`), `switch`
(strings too) as `match`, `for (Player p : players)` loops, locals with their Java names, string concatenation as
`format!`, the plugin's fields, and the plugin's own methods as Rust functions (translated the same way, as they
are called). A body translates whole or not at all: a `try`, another kind of loop, a call or a type with no Rust
form here leaves it a `TODO` saying why.

Then paper2horizon runs `cargo check` on the crate and turns every translation that doesn't compile back into a
`TODO` (with the translation as a comment to finish), until the crate compiles. `--no-check` skips that.

```rust
// Java: switch (args[0].toLowerCase()) { case "all": case "everyone": for (Player p : Bukkit.getOnlinePlayers()) ...
ctx.set_executor("greet", |_plugin, _ctx, sender, _label, args| {
    if args.is_empty() {
        return false;
    }
    match args[0].to_lowercase().as_str() {
        "all" | "everyone" => {
            for p in server().online_players() {
                p.send_message(&format!("{}Hello from {}!", ChatColor::Green, sender.name()));
            }
        }
        "me" => {
            sender.send_message(&format!("Hello, {}.", sender.name()));
        }
        _ => {
            sender.send_message(&format!("{}Unknown: {}", ChatColor::Red, args[0]));
            return true;
        }
    }
    true
});
```

The plugin's collections become Rust's, from the fields' generic types (`Map<UUID, Long>` → `HashMap<Uuid, i64>`),
and what Java may give as `null` is an `Option`:

```rust
let Some(target) = server().match_player(&args[0]) else {
    sender.send_message(&format!("{}{} isn't online.", ChatColor::Red, args[0]));
    return true;
};
let last = plugin.last_seen.get(&target.uuid).cloned();
if let Some(last) = last {
```

Read translated code over: Java's `null` and Rust's defaults differ (`getConfig().getString(path)` is `""` here when
the key is missing, not `null`), and a value Java reads without a check (`map.get(k).longValue()`) is `map[&k]`, which
panics where Java would throw (the plugin's runtime catches and logs both). A listener's or a command's own fields,
and the static fields of the plugin's classes, are fields of the plugin's struct too; `Main.getInstance()` is `plugin`.
On 66 small plugins from Modrinth, 26 of 277 bodies translate today, and every crate compiles; much of the rest keeps
its state in objects of the plugin's own classes (a `SeatManager`, a `FileManager`), which is next (TODO.md).

## Use

You need [Rust](https://rustup.rs).

```sh
cargo install --git https://github.com/JoaoVictorCoder/paper2horizon
paper2horizon MyPlugin.jar            # writes ./myplugin/
cd myplugin && cargo build --release  # target/release/libmyplugin.so goes in the server's plugins/
```

| Option | |
|---|---|
| `-o, --out <dir>` | where to write the crate (default: the plugin's name) |
| `--api-path <dir>` | use a local `horizon-server-plugins/api` instead of the git dependency |
| `--report` | print `PORTING.md` only, write nothing |
| `-f, --force` | write into a folder that isn't empty |

An example of what it makes, from the test plugin in `tests/fixtures/greeter`:

```rust
// com.example.greeter.Greeter.onBreak(BlockBreakEvent)
ctx.listen(EventPriority::High, true, |_plugin, _ctx, _e: &mut BlockBreakEvent| {
    // TODO: port onBreak.
    // The Java code calls:
    //   BlockBreakEvent.getBlock     → e.block
    //   Block.getType                → block.kind ("minecraft:stone"), or block.is("stone")
    //   BlockBreakEvent.getPlayer    → e.player
    //   Player.hasPermission         → player.has_permission(node)
    //   BlockBreakEvent.setCancelled → e.set_cancelled(true)
    //   Player.sendMessage           → player.send_message(text)
    // Constants: Material.BEDROCK → "minecraft:bedrock", ChatColor.RED → ChatColor::Red
    // Text: "greeter.bedrock"
    //       "{}You can't break bedrock."
});
```

## How it works

- `src/classfile.rs` reads `.class` files: the class, its methods and annotations, and what each method's code
  calls, reads, creates and holds as text (lambdas and string concatenation included).
- `src/analysis.rs` finds the listeners (`@EventHandler`, and `registerEvent` as Kotlin plugins use it), which
  class each `getCommand(name).setExecutor(...)` hands a command to, the tasks `onEnable` schedules, and every
  Bukkit call. Jars inside the jar are read too; libraries bundled or relocated into the plugin are told apart.
- `src/mapping.rs` holds what Bukkit and Paper name in horizon-plugin-api: events, methods and constants. A new
  part of the API means a new line there; a test checks that every event it maps to exists in the API.
- `src/translate.rs` translates method bodies (above); `src/verify.rs` runs `cargo check` and finds the lines that
  fail.
- `src/generate.rs` writes the crate.

Tried on CoreProtect, LuckPerms, ViaVersion, WorldEdit, TAB, Chunky, Simple Voice Chat, AxGraves and VeinMiner,
and on 66 small plugins: each converts, and each crate compiles. The big ones translate no bodies yet: their
listeners hand everything to their own frameworks.

## Development

```sh
cargo test                    # the class reader, the tables, and a conversion of the test plugin
cargo test -- --ignored       # also builds the converted test plugin (needs ../horizon-server-plugins)
tests/fixtures/greeter/build.sh  # rebuilds the test plugin's jar (needs a JDK) after changing it
```

The test plugin is compiled against small stubs of Bukkit's classes (`tests/fixtures/stubs`), under Bukkit's own
names, so its bytecode is what a real plugin's is without downloading Paper.

## License

MIT. Paper, Bukkit and Spigot are their authors'; this project is not affiliated with them, with Mojang or
Microsoft. It reads plugins you have, and copies nothing of theirs but their `plugin.yml` and resources into the
crate it writes for you.
