//! Converts tests/fixtures/greeter/greeter.jar (a small Paper plugin, built against Bukkit stubs by
//! its build.sh) and checks what comes out.

use paper2horizon::{analysis, generate, mapping};
use std::path::Path;

fn greeter() -> analysis::Analysis {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/greeter/greeter.jar");
    analysis::analyze(analysis::read_jar(&jar).unwrap()).unwrap()
}

#[test]
fn listeners_keep_their_event_priority_and_flags() {
    let a = greeter();
    let find = |m: &str| a.listeners.iter().find(|l| l.method == m).unwrap();
    let join = find("onJoin");
    assert_eq!(mapping::simple_name(&join.event), "PlayerJoinEvent");
    assert_eq!((join.priority.as_str(), join.ignore_cancelled), ("NORMAL", false));
    let brk = find("onBreak");
    assert_eq!((brk.priority.as_str(), brk.ignore_cancelled), ("HIGH", true));
    assert!(
        brk.uses.constants.iter().any(|(f, h)| f.name == "BEDROCK" && h.as_deref() == Some("\"minecraft:bedrock\""))
    );
    let quit = find("onQuit");
    assert_eq!(quit.class, "com/example/greeter/DamageListener");
    assert_eq!(quit.priority, "MONITOR");
    assert!(mapping::event("EntityDamageEvent").unwrap().horizon.is_none());
}

#[test]
fn commands_find_their_executors() {
    let a = greeter();
    let heal = a.commands.iter().find(|c| c.name == "heal").unwrap();
    assert_eq!(heal.executor.as_ref().unwrap().0, "com/example/greeter/HealCommand");
    assert_eq!(heal.completer.as_ref().unwrap().0, "com/example/greeter/HealCommand");
    let spawn = a.commands.iter().find(|c| c.name == "spawn").unwrap();
    // `getCommand("spawn").setExecutor(this)`: the plugin itself.
    assert_eq!(spawn.executor.as_ref().unwrap().0, "com/example/greeter/Greeter");
    assert!(spawn.completer.is_none());
}

#[test]
fn on_enable_holds_its_task_and_config() {
    let a = greeter();
    let e = a.on_enable.as_ref().unwrap();
    assert!(e.calls_save_default_config);
    assert_eq!(e.tasks.len(), 1);
    assert_eq!(e.tasks[0].call.name, "runTaskTimer");
    assert_eq!(e.tasks[0].ticks, [20, 1200]);
    assert!(e.tasks[0].uses.calls.iter().any(|u| u.call.name == "broadcastMessage"));
    // The task's calls are under the task, the command names under the commands.
    assert!(!e.uses.calls.iter().any(|u| u.call.name == "broadcastMessage"));
    assert!(!e.uses.strings.iter().any(|s| s == "heal"));
    let other: Vec<&str> = a.other.iter().map(|o| o.1.as_str()).collect();
    assert_eq!(other, ["greet", "color"]);
}

#[test]
fn the_crate_has_every_part() {
    let a = greeter();
    let generated = generate::lib_rs(&a, true, &Default::default());
    let lib = generated.lib_rs.clone();
    for want in [
        "ctx.save_default_config(include_str!(\"../config.yml\"));",
        "pub joins: i32,",
        "use std::collections::{HashMap, HashSet};",
        "pub last_seen: HashMap<Uuid, i64>,",
        "pub muted: HashSet<Uuid>,",
        "left out, no Horizon event for EntityDamageEvent",
        "ctx.set_executor(\"heal\"",
        "ctx.set_executor(\"spawn\"",
        "horizon_plugin!(Greeter::default(), plugin_yml!());",
    ] {
        assert!(lib.contains(want), "lib.rs lacks {want}\n{lib}");
    }
    let md = generate::porting_md(&a, "greeter.jar", &generated);
    assert!(md.contains("| Listeners | 6 (5 with a Horizon event) |"), "{md}");
    assert!(
        md.contains("| Translated to Rust | 12 of 14 listeners, commands, completers, tasks and helpers |"),
        "{md}"
    );
    assert!(md.contains("`Player.setHealth`"));
    assert!(!md.contains("<init>"));
}

/// The bodies, translated: each listener, the completer and the task, line for line.
#[test]
fn bodies_are_translated() {
    let a = greeter();
    let lib = generate::lib_rs(&a, true, &Default::default()).lib_rs;
    let expect = [
        // onJoin: a field, and a helper of the plugin's own.
        "        ctx.on(|plugin, ctx, e: &mut PlayerJoinEvent| {
            plugin.joins += 1;
            greet(plugin, ctx, &e.player);
            e.join_message = None;
        });",
        // The helpers, as functions: one calling the other, its unused parameters marked.
        "fn greet(plugin: &mut Greeter, ctx: &mut Ctx<Greeter>, player: &Player) {
    let text = ctx.config().string_or(\"welcome\", \"\");
    player.send_message(&texts_color(plugin, ctx, &text.replace(\"%player%\", &player.name)));
}",
        "fn texts_color(_plugin: &mut Greeter, _ctx: &mut Ctx<Greeter>, text: &str) -> String {
    color::translate('&', text)
}",
        // onBreak: two conditions as one if, a constant compared, a cancel.
        "        ctx.listen(EventPriority::High, true, |_plugin, _ctx, e: &mut BlockBreakEvent| {
            if e.block.kind == \"minecraft:bedrock\" && !e.player.has_permission(\"greeter.bedrock\") {
                e.set_cancelled(true);
                e.player.send_message(&format!(\"{}You can't break bedrock.\", ChatColor::Red));
            }
        });",
        "        ctx.listen(EventPriority::Monitor, false, |_plugin, _ctx, e: &mut PlayerQuitEvent| {
            e.quit_message = Some(format!(\"Bye {}\", e.player.name));
        });",
        // The task with its ticks, reading the plugin's field.
        "        ctx.run_timer(20, 1200, |plugin, _ctx| {
            server().broadcast_message(&format!(\"{}Joins so far: {}\", ChatColor::Yellow, plugin.joins));
        });",
        "        ctx.set_tab_completer(\"heal\", |_plugin, _ctx, _sender, _label, _args| {
            vec![\"all\".to_owned(), \"me\".to_owned()]
        });",
        // A string switch with a shared case, a loop over the players, an early return.
        "        ctx.set_executor(\"greet\", |_plugin, _ctx, sender, _label, args| {
            if args.is_empty() {
                return false;
            }
            match args[0].to_lowercase().as_str() {
                \"all\" | \"everyone\" => {
                    for p in server().online_players() {
                        p.send_message(&format!(\"{}Hello from {}!\", ChatColor::Green, sender.name()));
                    }
                }
                \"me\" => {
                    sender.send_message(&format!(\"Hello, {}.\", sender.name()));
                }
                _ => {
                    sender.send_message(&format!(\"{}Unknown: {}\", ChatColor::Red, args[0]));
                    return true;
                }
            }
            true
        });",
        // State in the plugin's collections.
        "        ctx.on(|plugin, _ctx, e: &mut PlayerQuitEvent| {
            let id = e.player.uuid;
            plugin.last_seen.insert(id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64));
            plugin.muted.remove(&id);
        });",
        "            if plugin.muted.contains(&e.player.uuid) {
                e.set_cancelled(true);
            } else if e.message.contains(\"spam\") {
                plugin.muted.insert(e.player.uuid);
            }",
        // What may be null: `let ... else` when the null case leaves, `if let` otherwise.
        "            let Some(target) = server().match_player(&args[0]) else {
                sender.send_message(&format!(\"{}{} isn't online.\", ChatColor::Red, args[0]));
                return true;
            };
            let last = plugin.last_seen.get(&target.uuid).cloned();
            if let Some(last) = last {",
        // A list made in the body.
        "            let mut names: Vec<String> = Vec::new();
            for p in server().online_players() {
                if !plugin.muted.contains(&p.uuid) {
                    names.push(p.name.clone());
                }
            }
            names",
    ];
    for want in expect {
        assert!(lib.contains(want), "lib.rs lacks\n{want}\n\n{lib}");
    }
    // What can't be translated says why.
    assert!(lib.contains("Not translated: World.getSpawnLocation has no Rust form here."), "{lib}");
    assert!(lib.contains("Not translated: a value is left at a branch's end (a ?: expression)."), "{lib}");
}

/// Builds the converted crate against the API checked out beside this repository
/// (`../horizon-server-plugins/api`). Slow and needs that checkout: `cargo test -- --ignored`.
#[test]
#[ignore]
fn the_crate_compiles() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-server-plugins/api");
    let api = std::fs::canonicalize(&api).expect("horizon-server-plugins beside this repository");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("greeter-crate");
    let _ = std::fs::remove_dir_all(&out);
    generate::write_crate(
        &greeter(),
        "greeter.jar",
        &out,
        &generate::ApiSource::Path(api.display().to_string()),
        &Default::default(),
    )
    .unwrap();
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "--quiet"])
        .env("CARGO_TARGET_DIR", Path::new(env!("CARGO_TARGET_TMPDIR")).join("greeter-target"))
        .env("RUSTFLAGS", "-D warnings")
        .current_dir(&out)
        .status()
        .unwrap();
    assert!(status.success());
}
