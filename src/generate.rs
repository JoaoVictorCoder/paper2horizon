//! The crate a conversion writes: `Cargo.toml`, `plugin.yml`, the plugin's resources, `src/lib.rs`
//! with a listener for each `@EventHandler` and an executor for each command, and `PORTING.md`.

use crate::analysis::{Analysis, Command, Lifecycle, Listener, Uses};
use crate::mapping::{self, Port};
use std::fmt::Write as _;
use std::path::Path;

/// Where the plugin API comes from in the crate's `Cargo.toml`.
pub enum ApiSource {
    Git,
    /// A local checkout of horizon-server-plugins' `api/`.
    Path(String),
}

pub const API_GIT: &str = "https://github.com/JoaoVictorCoder/horizon-server-plugins";

/// `com/example/Greeter` → `com.example.Greeter`.
pub fn java_name(internal: &str) -> String {
    internal.replace('/', ".")
}

/// The crate's name: the plugin's, lower case with dashes.
pub fn crate_name(plugin: &str) -> String {
    let mut out = String::new();
    for c in plugin.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) { format!("plugin-{out}") } else { out }
}

/// The plugin's type name in Rust.
pub fn struct_name(plugin: &str) -> String {
    let words: String = plugin
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect();
    if words.is_empty() || words.starts_with(|c: char| c.is_ascii_digit()) { format!("Plugin{words}") } else { words }
}

fn short(text: &str) -> String {
    let one_line = text.replace('\n', "\\n").replace('\r', "");
    if one_line.chars().count() > 90 { one_line.chars().take(87).collect::<String>() + "..." } else { one_line }
}

/// Comment lines saying what Java code calls and what each call becomes, `indent` spaces in.
fn notes(uses: &Uses, indent: usize, skip: &[&str]) -> Vec<String> {
    let pad = " ".repeat(indent);
    let mut out = Vec::new();
    let (internal, calls): (Vec<_>, Vec<_>) = uses
        .calls
        .iter()
        .filter(|u| !skip.contains(&u.call.name.as_str()))
        .partition(|u| crate::analysis::is_internal(&u.call.owner));
    if !calls.is_empty() {
        out.push(format!("{pad}// The Java code calls:"));
        let width =
            calls.iter().map(|u| mapping::simple_name(&u.call.owner).len() + u.call.name.len() + 1).max().unwrap_or(0);
        for u in calls {
            let what = format!("{}.{}", mapping::simple_name(&u.call.owner), u.call.name);
            let sender = matches!(mapping::simple_name(&u.call.owner), "CommandSender" | "ConsoleCommandSender");
            let line = match u.port {
                Some(Port::To(rust)) if sender => {
                    format!("{what:<width$} → {}", rust.replacen("player.", "sender.", 1))
                }
                Some(Port::To(rust)) => format!("{what:<width$} → {rust}"),
                Some(Port::Missing(why)) => format!("{what:<width$} ✗ {why}"),
                None => format!("{what:<width$} ? no Horizon equivalent known"),
            };
            out.push(format!("{pad}//   {line}"));
        }
    }
    if !internal.is_empty() {
        out.push(format!(
            "{pad}// ✗ And {} calls into the server's internals (Minecraft's code, CraftBukkit): no equivalent here.",
            internal.len()
        ));
    }
    let constants: Vec<String> = uses
        .constants
        .iter()
        .filter(|(f, _)| !crate::analysis::is_internal(&f.owner))
        .map(|(f, h)| match h {
            Some(h) => format!("{}.{} → {h}", mapping::simple_name(&f.owner), f.name),
            None => format!("{}.{}", mapping::simple_name(&f.owner), f.name),
        })
        .collect();
    if !constants.is_empty() {
        out.push(format!("{pad}// Constants: {}", constants.join(", ")));
    }
    for (i, s) in uses.strings.iter().take(12).enumerate() {
        let lead = if i == 0 { "// Text:" } else { "//      " };
        out.push(format!("{pad}{lead} \"{}\"", short(s)));
    }
    if uses.strings.len() > 12 {
        out.push(format!("{pad}//       ... and {} more", uses.strings.len() - 12));
    }
    out
}

fn priority(java: &str) -> String {
    mapping::camel(&java.to_ascii_lowercase())
}

fn push_lines(out: &mut String, lines: &[String]) {
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
}

/// Calls in `onEnable` that the generated `on_enable` already stands for.
const REGISTRATION: &[&str] = &[
    "registerEvents",
    "getPluginManager",
    "getCommand",
    "setExecutor",
    "setTabCompleter",
    "getServer",
    "getScheduler",
    "saveDefaultConfig",
    "runTask",
    "runTaskLater",
    "runTaskTimer",
    "runTaskAsynchronously",
    "runTaskLaterAsynchronously",
    "runTaskTimerAsynchronously",
    "scheduleSyncDelayedTask",
    "scheduleSyncRepeatingTask",
];

fn listener(out: &mut String, l: &Listener) {
    let event = mapping::simple_name(&l.event);
    let from = if l.dynamic {
        format!(
            "registered in code by {}.{} (registerEvent): port what its executor runs",
            java_name(&l.class),
            l.method
        )
    } else {
        format!("{}.{}({event})", java_name(&l.class), l.method)
    };
    match mapping::event(event).and_then(|m| m.horizon.map(|h| (h, m.note))) {
        Some((horizon, note)) => {
            let _ = writeln!(out, "        // {from}");
            if !note.is_empty() {
                let _ = writeln!(out, "        // {note}");
            }
            let p = priority(&l.priority);
            if p == "Normal" && !l.ignore_cancelled {
                let _ = writeln!(out, "        ctx.on(|_plugin, _ctx, _e: &mut {horizon}| {{");
            } else {
                let _ = writeln!(
                    out,
                    "        ctx.listen(EventPriority::{p}, {}, |_plugin, _ctx, _e: &mut {horizon}| {{",
                    l.ignore_cancelled
                );
            }
            let _ = writeln!(out, "            // TODO: port {}.", l.method);
            push_lines(out, &notes(&l.uses, 12, &[]));
            let _ = writeln!(out, "        }});\n");
        }
        None => {
            let why = mapping::event(event).map(|m| m.note).unwrap_or("No Horizon event yet.");
            let _ = writeln!(out, "        // {from}: left out, no Horizon event for {event}.");
            let _ = writeln!(out, "        // {why}");
            push_lines(out, &notes(&l.uses, 8, &[]));
            out.push('\n');
        }
    }
}

fn command(out: &mut String, c: &Command) {
    let name = &c.name;
    let from = match &c.executor {
        Some((class, _)) => format!("{}.onCommand", java_name(class)),
        None => "no executor found in the jar".to_owned(),
    };
    let _ = writeln!(out, "        // /{name}: {from}");
    if c.declared {
        let _ = writeln!(out, "        ctx.set_executor(\"{name}\", |_plugin, _ctx, _sender, _label, _args| {{");
    } else {
        let _ = writeln!(
            out,
            "        // Not in plugin.yml: registered here (add it to plugin.yml to give it a usage and permission)."
        );
        let _ = writeln!(
            out,
            "        ctx.command(CommandSpec::new(\"{name}\"), |_plugin, _ctx, _sender, _label, _args| {{"
        );
    }
    let _ = writeln!(out, "            // TODO: port the command. `_sender.player()` is Some for a player.");
    if let Some((_, uses)) = &c.executor {
        push_lines(out, &notes(uses, 12, &[]));
    }
    let _ = writeln!(out, "            true // false shows the command's usage");
    let _ = writeln!(out, "        }});");
    if let Some((class, uses)) = &c.completer {
        let _ = writeln!(out, "        // {}.onTabComplete", java_name(class));
        let _ = writeln!(out, "        ctx.set_tab_completer(\"{name}\", |_plugin, _ctx, _sender, _label, _args| {{");
        let _ =
            writeln!(out, "            // TODO: port the completions (the last of `_args` is the one being typed).");
        push_lines(out, &notes(uses, 12, &[]));
        let _ = writeln!(out, "            Vec::new()");
        let _ = writeln!(out, "        }});");
    }
    out.push('\n');
}

fn task(out: &mut String, call: &crate::classfile::MemberRef, uses: &Uses) {
    let open = match call.name.as_str() {
        "runTask" => "ctx.run_task(|_plugin, _ctx| {".to_owned(),
        "runTaskLater" | "scheduleSyncDelayedTask" => {
            "ctx.run_later(20, |_plugin, _ctx| { // TODO: the delay in ticks the Java code passes".to_owned()
        }
        "runTaskTimer" | "scheduleSyncRepeatingTask" => {
            "ctx.run_timer(20, 20, |_plugin, _ctx| { // TODO: the delay and period in ticks the Java code passes"
                .to_owned()
        }
        n if n.contains("Asynchronously") => {
            "ctx.run_async(|_sync| { // off the server thread: come back with _sync.run(|plugin, ctx| ...)".to_owned()
        }
        _ => "ctx.run_task(|_plugin, _ctx| {".to_owned(),
    };
    let _ = writeln!(out, "        // {}.{}", mapping::simple_name(&call.owner), call.name);
    let _ = writeln!(out, "        {open}");
    let _ = writeln!(out, "            // TODO: port the task.");
    push_lines(out, &notes(uses, 12, &[]));
    let _ = writeln!(out, "        }});\n");
}

fn lifecycle(out: &mut String, fn_name: &str, java: &str, main: &str, l: &Lifecycle) {
    let _ = writeln!(out, "    fn {fn_name}(&mut self, ctx: &mut Ctx<Self>) {{");
    let rest = notes(&l.uses, 8, REGISTRATION);
    if !rest.is_empty() {
        let _ = writeln!(out, "        // TODO: port the rest of {}.{java}.", java_name(main));
        push_lines(out, &rest);
    }
    let _ = writeln!(out, "        let _ = ctx;");
    let _ = writeln!(out, "    }}\n");
}

pub fn lib_rs(a: &Analysis, has_config: bool) -> String {
    let name = struct_name(&a.description.name);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "//! {} {}, converted from its Paper plugin by paper2horizon.",
        a.description.name, a.description.version
    );
    let _ = writeln!(out, "//!");
    let _ =
        writeln!(out, "//! Each listener, command and task below stands where the Java one was, with a TODO and what");
    let _ = writeln!(out, "//! the Java code called, mapped to horizon-plugin-api. PORTING.md has the whole list.\n");
    let _ = writeln!(out, "use horizon_plugin_api::prelude::*;\n");
    let _ = writeln!(out, "/// The plugin (Java: {}). TODO: its fields.", java_name(&a.main));
    let _ = writeln!(out, "#[derive(Default)]");
    let _ = writeln!(out, "pub struct {name} {{}}\n");
    let _ = writeln!(out, "impl Plugin for {name} {{");
    if let Some(l) = &a.on_load {
        lifecycle(&mut out, "on_load", "onLoad", &a.main, l);
    }
    let _ = writeln!(out, "    fn on_enable(&mut self, ctx: &mut Ctx<Self>) {{");
    if let Some(l) = &a.on_enable {
        if l.calls_save_default_config && has_config {
            let _ = writeln!(out, "        ctx.save_default_config(include_str!(\"../config.yml\"));\n");
        }
        let rest = notes(&l.uses, 8, REGISTRATION);
        if !rest.is_empty() {
            let _ = writeln!(out, "        // TODO: port the rest of {}.onEnable.", java_name(&a.main));
            push_lines(&mut out, &rest);
            out.push('\n');
        }
        for (call, uses) in &l.tasks {
            task(&mut out, call, uses);
        }
    }
    for l in &a.listeners {
        listener(&mut out, l);
    }
    for c in &a.commands {
        command(&mut out, c);
    }
    if a.brigadier {
        let _ = writeln!(
            out,
            "        // TODO: the plugin registers Brigadier commands (Paper's LifecycleEvents.COMMANDS): add each with"
        );
        let _ = writeln!(out, "        // ctx.command(CommandSpec::new(\"name\"), ...) and ctx.set_tab_completer.\n");
    }
    // Remove the blank line before the closing brace.
    while out.ends_with("\n\n") {
        out.pop();
    }
    let _ = writeln!(out, "    }}");
    if let Some(l) = &a.on_disable {
        out.push('\n');
        lifecycle(&mut out, "on_disable", "onDisable", &a.main, l);
        while out.ends_with("\n\n") {
            out.pop();
        }
    }
    let _ = writeln!(out, "}}\n");
    let _ = writeln!(out, "horizon_plugin!({name}::default(), plugin_yml!());");
    out
}

pub fn cargo_toml(a: &Analysis, api: &ApiSource) -> String {
    let dep = match api {
        ApiSource::Git => format!("horizon-plugin-api = {{ git = \"{API_GIT}\" }}"),
        ApiSource::Path(p) => format!("horizon-plugin-api = {{ path = \"{p}\" }}"),
    };
    let version = {
        // Cargo wants semver: keep the digits of the plugin's version, padded to three parts.
        let parts: Vec<String> = a
            .description
            .version
            .split(['.', '-', '+', ' '])
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .filter(|p| !p.is_empty())
            .take(3)
            .collect();
        let mut parts = parts;
        while parts.len() < 3 {
            parts.push("0".into());
        }
        parts.join(".")
    };
    format!(
        "[package]\nname = \"{}\"\nversion = \"{version}\"\nedition = \"2024\"\ndescription = \"{}\"\n\n\
         # The library a Horizon server loads from plugins/.\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n{dep}\n",
        crate_name(&a.description.name),
        a.description.description.replace('"', "'").replace('\n', " "),
    )
}

pub fn porting_md(a: &Analysis, jar: &str) -> String {
    let d = &a.description;
    let mut out = String::new();
    let _ = writeln!(out, "# Porting {} {} to Horizon\n", d.name, d.version);
    let _ = writeln!(
        out,
        "Made by paper2horizon from `{jar}`. Paper plugins are Java; this crate is the same plugin's shape in Rust for \
         [horizon-plugin-api]({API_GIT}): `plugin.yml`, a listener for each `@EventHandler`, an executor for each \
         command, the tasks `onEnable` schedules. The code inside each one is a `TODO` listing what the Java code \
         called and what that is here. Port them one by one, then build:\n"
    );
    let _ = writeln!(
        out,
        "```sh\ncargo build --release\ncp target/release/lib{}.so /path/to/server/plugins/\n```\n",
        crate_name(&d.name).replace('-', "_")
    );

    let mapped_listeners = a
        .listeners
        .iter()
        .filter(|l| mapping::event(mapping::simple_name(&l.event)).and_then(|m| m.horizon).is_some())
        .count();
    let all_uses: Vec<&Uses> = a
        .listeners
        .iter()
        .map(|l| &l.uses)
        .chain(a.commands.iter().filter_map(|c| c.executor.as_ref().map(|(_, u)| u)))
        .chain(a.commands.iter().filter_map(|c| c.completer.as_ref().map(|(_, u)| u)))
        .chain([&a.on_enable, &a.on_disable, &a.on_load].into_iter().flatten().map(|l| &l.uses))
        .chain(a.other.iter().map(|(_, _, u)| u))
        .collect();
    let mut distinct = std::collections::BTreeMap::new();
    for u in &all_uses {
        for c in u.calls.iter().filter(|c| !crate::analysis::is_internal(&c.call.owner)) {
            distinct.insert((mapping::simple_name(&c.call.owner).to_owned(), c.call.name.clone()), c.port);
        }
    }
    let to = distinct.values().filter(|p| matches!(p, Some(Port::To(_)))).count();
    let missing = distinct.values().filter(|p| matches!(p, Some(Port::Missing(_)))).count();
    let unknown = distinct.values().filter(|p| p.is_none()).count();

    let _ = writeln!(out, "## Summary\n");
    let _ = writeln!(out, "| | |\n|---|---|");
    let _ = writeln!(out, "| Listeners | {} ({mapped_listeners} with a Horizon event) |", a.listeners.len());
    let _ = writeln!(
        out,
        "| Commands | {} |",
        if a.commands.is_empty() {
            "none".to_owned()
        } else {
            a.commands.iter().map(|c| format!("`/{}`", c.name)).collect::<Vec<_>>().join(", ")
        }
    );
    let tasks = a.on_enable.as_ref().map_or(0, |l| l.tasks.len());
    let _ = writeln!(out, "| Tasks scheduled in `onEnable` | {tasks} |");
    let _ = writeln!(
        out,
        "| Bukkit/Paper calls | {} different: {to} with an equivalent, {missing} missing in Horizon, {unknown} unknown |",
        distinct.len()
    );
    if !a.internals.is_empty() {
        let total: usize = a.internals.iter().map(|(_, n)| n).sum();
        let _ = writeln!(
            out,
            "| Server internals (NMS, CraftBukkit) | {total} references in {} classes: no equivalent |",
            a.internals.len()
        );
    }
    if !a.libraries.is_empty() {
        let libs: Vec<String> = a.libraries.iter().map(|(n, c)| format!("`{n}` ({c})")).collect();
        let _ = writeln!(out, "| Libraries bundled in the jar | {} |", libs.join(", "));
    }
    if !a.resources.is_empty() {
        let res: Vec<String> = a.resources.iter().map(|r| format!("`{}`", r.path)).collect();
        let _ = writeln!(out, "| Resources | {} |", res.join(", "));
    }
    out.push('\n');

    let deps: Vec<&String> = d.depend.iter().chain(&d.soft_depend).collect();
    if !deps.is_empty()
        || a.from_paper_yml
        || a.brigadier
        || !a.libraries.is_empty()
        || !a.unreadable.is_empty()
        || !a.internals.is_empty()
        || !a.nested.is_empty()
    {
        let _ = writeln!(out, "## Before you start\n");
        if !a.internals.is_empty() {
            let total: usize = a.internals.iter().map(|(_, n)| n).sum();
            let top: Vec<String> =
                a.internals.iter().take(5).map(|(c, n)| format!("`{}` ({n})", java_name(c))).collect();
            let _ = writeln!(
                out,
                "- **It reaches into the server's internals** (Minecraft's own code or CraftBukkit, {total} references in {} classes: {}). \
                 None of that carries over: Horizon is a different server. Those parts need the API, or a change to Horizon itself.",
                a.internals.len(),
                top.join(", ")
            );
        }
        if !a.nested.is_empty() {
            let _ = writeln!(out, "- Its code is in jars inside the jar ({}); those were read too.", list(&a.nested));
        }
        if !d.depend.is_empty() {
            let _ = writeln!(
                out,
                "- It needs {}: each must be a Horizon plugin too, or its part done another way.",
                list(&d.depend)
            );
        }
        if !d.soft_depend.is_empty() {
            let _ = writeln!(out, "- It works with {} when they are there (`softdepend`).", list(&d.soft_depend));
        }
        if a.from_paper_yml {
            let _ = writeln!(
                out,
                "- The jar has `paper-plugin.yml`, no `plugin.yml`: the one here keeps its name, version and description. Add `commands:` for the commands it registers in code."
            );
        }
        if a.brigadier {
            let _ = writeln!(
                out,
                "- It registers commands with Brigadier (Paper's `LifecycleEvents.COMMANDS`): add them with `ctx.command` and `ctx.set_tab_completer`."
            );
        }
        if !a.libraries.is_empty() {
            let _ = writeln!(
                out,
                "- Libraries bundled in the jar are Java: use Rust crates for what they did (bStats has no Horizon side; Gson → serde_json; HikariCP/JDBC → sqlx or rusqlite)."
            );
        }
        for (c, e) in &a.unreadable {
            let _ = writeln!(out, "- `{c}` couldn't be read: {e}.");
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Listeners\n");
    if a.listeners.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(out, "| Java | Event | Horizon | Priority |\n|---|---|---|---|");
        for l in &a.listeners {
            let e = mapping::simple_name(&l.event);
            let h = match mapping::event(e) {
                Some(m) => match m.horizon {
                    Some(h) if m.note.is_empty() => format!("`{h}`"),
                    Some(h) => format!("`{h}`: {}", m.note),
                    None => format!("✗ {}", m.note),
                },
                None => "✗ No Horizon event yet.".to_owned(),
            };
            let ignore = if l.ignore_cancelled {
                ", ignoreCancelled"
            } else if l.dynamic {
                " (registerEvent)"
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "| `{}.{}` | `{e}` | {h} | {}{ignore} |",
                mapping::simple_name(&l.class),
                l.method,
                l.priority.to_ascii_lowercase()
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Commands\n");
    if a.commands.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(out, "| Command | Executor | Tab completion |\n|---|---|---|");
        for c in &a.commands {
            let ex = c.executor.as_ref().map_or("✗ not found".to_owned(), |(k, _)| format!("`{}`", java_name(k)));
            let tab = c
                .completer
                .as_ref()
                .map_or("players' names (Horizon's default)".to_owned(), |(k, _)| format!("`{}`", java_name(k)));
            let declared = if c.declared { "" } else { " (registered in code, not in plugin.yml)" };
            let _ = writeln!(out, "| `/{}`{declared} | {ex} | {tab} |", c.name);
        }
        out.push('\n');
    }

    let missing_list: Vec<_> = distinct
        .iter()
        .filter_map(|(k, p)| match p {
            Some(Port::Missing(why)) => Some((k, *why)),
            _ => None,
        })
        .collect();
    if !missing_list.is_empty() {
        let _ = writeln!(out, "## Missing in Horizon\n");
        let _ =
            writeln!(out, "What the plugin calls that horizon-plugin-api doesn't have yet, and what comes closest:\n");
        for ((owner, name), why) in missing_list {
            let _ = writeln!(out, "- `{owner}.{name}`: {why}");
        }
        out.push('\n');
    }
    let unknown_list: Vec<_> = distinct.iter().filter(|(_, p)| p.is_none()).map(|(k, _)| k).collect();
    if !unknown_list.is_empty() {
        let _ = writeln!(out, "## Calls with no known equivalent\n");
        let _ = writeln!(
            out,
            "Not in paper2horizon's tables. Look for them in the API's docs (docs/plugins.md), or do without:\n"
        );
        let _ = writeln!(
            out,
            "{}\n",
            unknown_list.iter().map(|(o, n)| format!("`{o}.{n}`")).collect::<Vec<_>>().join(", ")
        );
    }

    if !a.other.is_empty() {
        let _ = writeln!(out, "## Other code that uses the API\n");
        let _ = writeln!(
            out,
            "Methods outside the listeners, commands and `onEnable`, to port where they are called from:\n"
        );
        let shown: Vec<_> = a
            .other
            .iter()
            .filter(|(_, _, u)| u.calls.iter().any(|c| !crate::analysis::is_internal(&c.call.owner)))
            .collect();
        if shown.len() > 60 {
            let _ = writeln!(out, "The first 60 of {}.\n", shown.len());
        }
        for (class, method, uses) in shown.into_iter().take(60) {
            let _ = writeln!(out, "### `{}.{method}`\n", java_name(class));
            let _ = writeln!(out, "```text");
            for l in notes(uses, 0, &[]) {
                let _ = writeln!(out, "{}", l.trim_start_matches("// "));
            }
            let _ = writeln!(out, "```\n");
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn list(names: &[String]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

/// Writes the crate into `dir`.
pub fn write_crate(a: &Analysis, jar: &str, dir: &Path, api: &ApiSource) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    let mut write = |rel: &str, bytes: &[u8]| -> Result<(), String> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(rel.to_owned());
        Ok(())
    };
    let has_config = a.resources.iter().any(|r| r.path == "config.yml");
    write("Cargo.toml", cargo_toml(a, api).as_bytes())?;
    write(".gitignore", b"/target\n")?;
    write("plugin.yml", a.plugin_yml.as_bytes())?;
    for r in &a.resources {
        // config.yml beside Cargo.toml, for include_str!("../config.yml"); the others under resources/.
        let rel = if r.path == "config.yml" { "config.yml".to_owned() } else { format!("resources/{}", r.path) };
        write(&rel, &r.bytes)?;
    }
    write("src/lib.rs", lib_rs(a, has_config).as_bytes())?;
    write("PORTING.md", porting_md(a, jar).as_bytes())?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_rust_names() {
        assert_eq!(crate_name("Greeter"), "greeter");
        assert_eq!(crate_name("My Cool_Plugin!"), "my-cool-plugin");
        assert_eq!(crate_name("123"), "plugin-123");
        assert_eq!(struct_name("my-cool plugin"), "MyCoolPlugin");
        assert_eq!(struct_name("9lives"), "Plugin9lives");
    }
}
