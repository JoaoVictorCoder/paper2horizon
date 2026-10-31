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
    assert_eq!(e.tasks[0].0.name, "runTaskTimer");
    assert!(e.tasks[0].1.calls.iter().any(|u| u.call.name == "broadcastMessage"));
    // The task's calls are under the task, the command names under the commands.
    assert!(!e.uses.calls.iter().any(|u| u.call.name == "broadcastMessage"));
    assert!(!e.uses.strings.iter().any(|s| s == "heal"));
    assert!(a.other.is_empty(), "{:?}", a.other.iter().map(|o| &o.1).collect::<Vec<_>>());
}

#[test]
fn the_crate_has_every_part() {
    let a = greeter();
    let lib = generate::lib_rs(&a, true);
    for want in [
        "ctx.save_default_config(include_str!(\"../config.yml\"));",
        "ctx.on(|_plugin, _ctx, _e: &mut PlayerJoinEvent|",
        "ctx.listen(EventPriority::High, true, |_plugin, _ctx, _e: &mut BlockBreakEvent|",
        "ctx.listen(EventPriority::Monitor, false, |_plugin, _ctx, _e: &mut PlayerQuitEvent|",
        "left out, no Horizon event for EntityDamageEvent",
        "ctx.set_executor(\"heal\"",
        "ctx.set_tab_completer(\"heal\"",
        "ctx.set_executor(\"spawn\"",
        "ctx.run_timer(",
        "sender.send_message(text)",
        "horizon_plugin!(Greeter::default(), plugin_yml!());",
    ] {
        assert!(lib.contains(want), "lib.rs lacks {want}\n{lib}");
    }
    let md = generate::porting_md(&a, "greeter.jar");
    assert!(md.contains("| Listeners | 4 (3 with a Horizon event) |"), "{md}");
    assert!(md.contains("`Player.setHealth`"));
    assert!(!md.contains("<init>"));
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
    generate::write_crate(&greeter(), "greeter.jar", &out, &generate::ApiSource::Path(api.display().to_string()))
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
