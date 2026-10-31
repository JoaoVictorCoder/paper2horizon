//! What Bukkit and Paper name, in horizon-plugin-api: events, then methods and constants.
//!
//! Each entry says what to write instead, or why there is nothing yet and what comes closest.
//! The tables follow `docs/plugins.md` of horizon-server-plugins; a new API there means a new line
//! here.

/// A Bukkit or Paper event and the Horizon event that matches it.
pub struct EventMap {
    /// The event's simple class name (`PlayerJoinEvent`), as Bukkit, Paper and Spigot name it.
    pub bukkit: &'static str,
    /// The Horizon event, or `None` when there is none yet.
    pub horizon: Option<&'static str>,
    /// What differs, or what to use instead.
    pub note: &'static str,
}

const fn ev(bukkit: &'static str, horizon: &'static str, note: &'static str) -> EventMap {
    EventMap { bukkit, horizon: Some(horizon), note }
}

const fn none(bukkit: &'static str, note: &'static str) -> EventMap {
    EventMap { bukkit, horizon: None, note }
}

pub const EVENTS: &[EventMap] = &[
    ev("PlayerJoinEvent", "PlayerJoinEvent", "`e.join_message`: `Some(text)`, or `None` for no message."),
    ev("PlayerQuitEvent", "PlayerQuitEvent", "`e.quit_message`: `Some(text)`, or `None` for no message."),
    ev(
        "PlayerMoveEvent",
        "PlayerMoveEvent",
        "Seen once a tick, not on every packet. Cancelling sends the player back to `e.from`.",
    ),
    ev("PlayerChangedWorldEvent", "PlayerChangedWorldEvent", "`e.from` is the world's name."),
    ev("PlayerDeathEvent", "PlayerDeathEvent", "No death message, drops or kept XP to change yet."),
    ev(
        "PlayerRespawnEvent",
        "PlayerRespawnEvent",
        "The respawn location can't be changed yet: teleport the player from `ctx.run_task`.",
    ),
    ev(
        "PlayerGameModeChangeEvent",
        "PlayerGameModeChangeEvent",
        "Seen after the change (`e.previous` is the old mode); it can't be cancelled.",
    ),
    ev(
        "AsyncPlayerChatEvent",
        "AsyncPlayerChatEvent",
        "`e.message` and `e.format` (`%1$s` the name, `%2$s` the message). Runs on the server thread.",
    ),
    ev(
        "AsyncChatEvent",
        "AsyncPlayerChatEvent",
        "Paper's Adventure chat event: here the message is plain text in `e.message`.",
    ),
    ev("PlayerChatEvent", "AsyncPlayerChatEvent", "The deprecated synchronous chat event."),
    ev(
        "PlayerInteractEvent",
        "PlayerInteractEvent",
        "`e.action` is `right_click_block`, `right_click_air` or `left_click_block` (no `left_click_air` yet); `e.block`, `e.face`, `e.item`.",
    ),
    ev("BlockBreakEvent", "BlockBreakEvent", "`e.block.kind` is `minecraft:stone`; drops and XP can't be changed yet."),
    ev("BlockPlaceEvent", "BlockPlaceEvent", "`e.block` is where it lands, `e.against`, `e.face`, `e.item`."),
    ev("PlayerBucketFillEvent", "PlayerBucketFillEvent", ""),
    ev("PlayerBucketEmptyEvent", "PlayerBucketEmptyEvent", ""),
    ev("PrePlayerAttackEntityEvent", "PrePlayerAttackEntityEvent", ""),
    ev(
        "EntityDamageByEntityEvent",
        "PrePlayerAttackEntityEvent",
        "Only blows from players, and only to cancel: the damage can't be read or changed yet.",
    ),
    ev("EntitySpawnEvent", "EntitySpawnEvent", ""),
    ev("CreatureSpawnEvent", "EntitySpawnEvent", "No spawn reason yet."),
    ev("EntityDeathEvent", "EntityDeathEvent", "No drops or XP to change yet."),
    ev("EntityExplodeEvent", "EntityExplodeEvent", "Take blocks out of `e.blocks` to spare them."),
    ev("BlockExplodeEvent", "EntityExplodeEvent", "Every explosion comes as `EntityExplodeEvent` here."),
    ev("BlockFromToEvent", "BlockFromToEvent", ""),
    ev("BlockSpreadEvent", "BlockSpreadEvent", "Fire spreading; other spreads (grass, mushrooms) don't come yet."),
    ev("BlockBurnEvent", "BlockSpreadEvent", "Fire burning a block comes as `BlockSpreadEvent`."),
    ev(
        "BlockIgniteEvent",
        "BlockSpreadEvent",
        "Only fire spreading by itself; flint and steel is a `PlayerInteractEvent`.",
    ),
    ev("BlockPistonExtendEvent", "BlockPistonEvent", "Pushing and pulling are one event: `e.direction`."),
    ev("BlockPistonRetractEvent", "BlockPistonEvent", "Pushing and pulling are one event: `e.direction`."),
    ev(
        "InventoryClickEvent",
        "InventoryClickEvent",
        "Only in menus a plugin opened (`Player::open_menu`); clicks there never move items.",
    ),
    ev("InventoryCloseEvent", "InventoryCloseEvent", "Only menus a plugin opened."),
    ev("ServerLoadEvent", "ServerLoadEvent", ""),
    ev("ServerTickStartEvent", "ServerTickStartEvent", ""),
    ev("ServerTickEndEvent", "ServerTickEndEvent", ""),
    ev("WorldLoadEvent", "WorldLoadEvent", "Once, for the server's world."),
    ev("WorldUnloadEvent", "WorldUnloadEvent", "Once, for the server's world."),
    ev("PluginEnableEvent", "PluginEnableEvent", "`e.plugin` is the plugin's name."),
    ev("PluginDisableEvent", "PluginDisableEvent", "`e.plugin` is the plugin's name."),
    none("PlayerTeleportEvent", "No event yet. `PlayerMoveEvent` sees the player arrive."),
    none("PlayerLoginEvent", "No event yet: `PlayerJoinEvent` comes once the player is in the world."),
    none(
        "AsyncPlayerPreLoginEvent",
        "No event yet: `PlayerJoinEvent` comes once the player is in the world (kick from there).",
    ),
    none("PlayerCommandPreprocessEvent", "No event yet: commands reach only their own executors."),
    none("PlayerDropItemEvent", "No event yet."),
    none("EntityPickupItemEvent", "No event yet."),
    none("PlayerPickupItemEvent", "No event yet."),
    none(
        "PlayerItemConsumeEvent",
        "No event yet: `PlayerInteractEvent` with `right_click_air` comes as the item is used.",
    ),
    none("PlayerInteractEntityEvent", "No event yet."),
    none("PlayerInteractAtEntityEvent", "No event yet."),
    none("PlayerToggleSneakEvent", "No event yet."),
    none("PlayerToggleSprintEvent", "No event yet."),
    none("PlayerToggleFlightEvent", "No event yet."),
    none("PlayerItemHeldEvent", "No event yet."),
    none("PlayerSwapHandItemsEvent", "No event yet."),
    none("PlayerKickEvent", "No event yet: `PlayerQuitEvent` comes after."),
    none("PlayerExpChangeEvent", "No event yet."),
    none("PlayerLevelChangeEvent", "No event yet."),
    none("FoodLevelChangeEvent", "No event yet."),
    none("EntityDamageEvent", "No event yet. Blows from players: `PrePlayerAttackEntityEvent`, to cancel them."),
    none("EntityRegainHealthEvent", "No event yet."),
    none("EntityTargetEvent", "No event yet."),
    none("EntityTargetLivingEntityEvent", "No event yet."),
    none("EntityChangeBlockEvent", "No event yet."),
    none("EntityCombustEvent", "No event yet."),
    none("ProjectileHitEvent", "No event yet."),
    none("ProjectileLaunchEvent", "No event yet."),
    none("ItemSpawnEvent", "No event yet: `EntitySpawnEvent` covers mobs only."),
    none("VehicleEnterEvent", "No event yet."),
    none("VehicleExitEvent", "No event yet."),
    none("InventoryOpenEvent", "No event yet."),
    none("InventoryDragEvent", "No event yet."),
    none("CraftItemEvent", "No event yet."),
    none("PrepareItemCraftEvent", "No event yet."),
    none("FurnaceSmeltEvent", "No event yet."),
    none("SignChangeEvent", "No event yet: there is no block entity data yet."),
    none("BlockDamageEvent", "No event yet: `PlayerInteractEvent` with `left_click_block` comes as digging starts."),
    none("BlockGrowEvent", "No event yet."),
    none("BlockFadeEvent", "No event yet."),
    none("LeavesDecayEvent", "No event yet."),
    none("BlockRedstoneEvent", "No event yet."),
    none("BlockPhysicsEvent", "No event yet."),
    none("ChunkLoadEvent", "No event yet (on Horizon's list)."),
    none("ChunkUnloadEvent", "No event yet (on Horizon's list)."),
    none("WeatherChangeEvent", "No event yet."),
    none("ServerListPingEvent", "No event yet."),
    none("PaperServerListPingEvent", "No event yet."),
    none("ServerCommandEvent", "No event yet."),
    none("TabCompleteEvent", "No event yet: give each command a completer with `ctx.set_tab_completer`."),
];

pub fn event(simple_name: &str) -> Option<&'static EventMap> {
    EVENTS.iter().find(|e| e.bukkit == simple_name)
}

/// What a Bukkit call becomes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Port {
    /// Write this instead.
    To(&'static str),
    /// Nothing equal yet; what comes closest.
    Missing(&'static str),
}

struct Call {
    owners: &'static [&'static str],
    name: &'static str,
    port: Port,
}

const PLAYER: &[&str] = &[
    "Player",
    "CommandSender",
    "HumanEntity",
    "LivingEntity",
    "Entity",
    "OfflinePlayer",
    "ConsoleCommandSender",
    "Permissible",
    "ServerOperator",
    "Damageable",
    "Audience",
];
const SERVER: &[&str] = &["Bukkit", "Server"];
const PLUGIN: &[&str] = &["JavaPlugin", "Plugin"];
const CONFIG: &[&str] = &[
    "FileConfiguration",
    "YamlConfiguration",
    "ConfigurationSection",
    "MemorySection",
    "MemoryConfiguration",
    "Configuration",
];
const SCHEDULER: &[&str] = &["BukkitScheduler", "BukkitRunnable"];
const LOGGER: &[&str] = &["Logger"];
const WORLD: &[&str] = &["World"];
const BLOCK: &[&str] = &["Block", "BlockState"];
const LOCATION: &[&str] = &["Location"];
const ITEM: &[&str] = &["ItemStack", "ItemMeta", "PlayerInventory", "Inventory"];
const EVENT: &[&str] = &["*Event"];

const fn to(owners: &'static [&'static str], name: &'static str, rust: &'static str) -> Call {
    Call { owners, name, port: Port::To(rust) }
}

const fn missing(owners: &'static [&'static str], name: &'static str, why: &'static str) -> Call {
    Call { owners, name, port: Port::Missing(why) }
}

const CALLS: &[Call] = &[
    // Players and command senders.
    to(PLAYER, "sendMessage", "player.send_message(text)"),
    to(PLAYER, "sendRichMessage", "player.send_message(text) (with § codes instead of MiniMessage)"),
    to(PLAYER, "hasPermission", "player.has_permission(node)"),
    to(PLAYER, "getName", "player.name"),
    to(PLAYER, "getDisplayName", "player.name"),
    to(PLAYER, "getUniqueId", "player.uuid"),
    to(PLAYER, "isOp", "player.is_op()"),
    to(PLAYER, "isOnline", "player.is_online()"),
    to(PLAYER, "isDead", "player.is_dead()"),
    to(PLAYER, "getLocation", "player.location"),
    to(PLAYER, "getWorld", "player.world()"),
    to(PLAYER, "teleport", "player.teleport(&location)"),
    to(PLAYER, "teleportAsync", "player.teleport(&location)"),
    to(PLAYER, "kickPlayer", "player.kick(reason)"),
    to(PLAYER, "kick", "player.kick(reason)"),
    to(PLAYER, "sendTitle", "player.send_title(title, subtitle, fade_in, stay, fade_out)"),
    to(PLAYER, "showTitle", "player.send_title(title, subtitle, fade_in, stay, fade_out)"),
    to(PLAYER, "sendActionBar", "player.send_action_bar(text)"),
    to(PLAYER, "setGameMode", "player.set_game_mode(GameMode::Creative)"),
    to(PLAYER, "getGameMode", "player.game_mode"),
    to(PLAYER, "getHealth", "player.health"),
    to(PLAYER, "getInventory", "player.inventory()"),
    to(PLAYER, "getItemInHand", "player.item_in_hand()"),
    to(PLAYER, "performCommand", "player.perform_command(line)"),
    to(PLAYER, "playSound", "player.play_sound(\"minecraft:entity.player.levelup\", volume, pitch)"),
    to(PLAYER, "addPotionEffect", "player.add_effect(\"minecraft:speed\", seconds, amplifier, particles)"),
    to(PLAYER, "removePotionEffect", "player.clear_effects() (all of them)"),
    to(PLAYER, "openInventory", "player.open_menu(&menu) (chest menus only)"),
    to(PLAYER, "closeInventory", "player.close_menu()"),
    to(PLAYER, "giveExpLevels", "player.give_levels(levels)"),
    to(PLAYER, "getFacing", "player.location.facing()"),
    missing(
        PLAYER,
        "setHealth",
        "No health setter yet: run a vanilla command (`/effect give <name> instant_health`) with server().dispatch_command.",
    ),
    missing(PLAYER, "getMaxHealth", "No max health yet: 20 for a player without attribute changes."),
    missing(PLAYER, "setFoodLevel", "No hunger yet: `/effect give <name> saturation` with server().dispatch_command."),
    missing(PLAYER, "setLevel", "Only player.give_levels(levels) (adds) for now."),
    missing(PLAYER, "setExp", "No XP setter yet: `/xp set` with server().dispatch_command."),
    missing(PLAYER, "setAllowFlight", "No flight yet: game mode or `/ability` mods only."),
    missing(PLAYER, "setFlying", "No flight yet."),
    missing(PLAYER, "hidePlayer", "No hiding players yet."),
    missing(PLAYER, "showPlayer", "No hiding players yet."),
    missing(
        PLAYER,
        "setDisplayName",
        "No display names yet: change the chat `format` in `AsyncPlayerChatEvent`, or use a Team prefix.",
    ),
    missing(PLAYER, "setPlayerListName", "No tab list names yet: a Team prefix shows there."),
    missing(PLAYER, "getAddress", "No player address yet."),
    missing(PLAYER, "setVelocity", "No velocity yet."),
    missing(PLAYER, "getTargetBlock", "No ray cast yet: player.location.facing() gives the direction."),
    missing(
        PLAYER,
        "getPersistentDataContainer",
        "No persistent data yet: keep it in a file in ctx.data_folder() (as Homes' homes.yml).",
    ),
    missing(PLAYER, "setMetadata", "No metadata yet: keep a HashMap by player.uuid in the plugin's struct."),
    missing(PLAYER, "getMetadata", "No metadata yet: keep a HashMap by player.uuid in the plugin's struct."),
    // The server.
    to(SERVER, "broadcastMessage", "server().broadcast_message(text)"),
    to(SERVER, "broadcast", "server().broadcast(text, permission)"),
    to(SERVER, "getOnlinePlayers", "server().online_players()"),
    to(SERVER, "getPlayer", "server().player(name) (server().player_by_uuid(uuid) for a UUID)"),
    to(SERVER, "getPlayerExact", "server().player(name)"),
    to(SERVER, "matchPlayer", "server().match_player(name)"),
    to(SERVER, "dispatchCommand", "server().dispatch_command(line) (runs as the console, at the next tick)"),
    to(SERVER, "getConsoleSender", "server().console_sender()"),
    to(SERVER, "getWorld", "server().world(name)"),
    to(SERVER, "getWorlds", "server().worlds()"),
    to(SERVER, "getVersion", "server().version()"),
    to(SERVER, "getBukkitVersion", "server().minecraft_version()"),
    to(SERVER, "getMinecraftVersion", "server().minecraft_version()"),
    to(SERVER, "getName", "server().name()"),
    to(SERVER, "getLogger", "ctx.logger()"),
    to(SERVER, "getScheduler", "ctx.run_task / run_later / run_timer / run_async"),
    to(SERVER, "getPluginManager", "(listeners: ctx.listen in on_enable; plugins: server().plugin(name))"),
    to(SERVER, "getCurrentTick", "server().current_tick()"),
    to(SERVER, "createBossBar", "BossBar::create(id, title)"),
    to(SERVER, "getScoreboardManager", "Objective / Team (horizon_plugin_api::scoreboard)"),
    to(SERVER, "savePlayers", "server().save_all()"),
    to(SERVER, "getServicesManager", "server().message_plugin(name, channel, value) and ctx.on_message(channel, ...)"),
    missing(SERVER, "getOfflinePlayer", "No offline players yet: keep what's needed by UUID in a file."),
    missing(SERVER, "createInventory", "Chest menus only: Menu::new(rows, title), shown with player.open_menu(&menu)."),
    missing(SERVER, "getBanList", "server().ban(name, reason) / server().pardon(name)."),
    missing(SERVER, "getMaxPlayers", "No server settings yet."),
    missing(SERVER, "getMotd", "No server settings yet."),
    missing(SERVER, "addRecipe", "No recipes yet: a data pack can add them."),
    to(&["PluginManager"], "registerEvents", "(each @EventHandler is a ctx.listen in on_enable)"),
    to(&["PluginManager"], "isPluginEnabled", "server().is_plugin_enabled(name)"),
    to(&["PluginManager"], "getPlugin", "server().plugin(name)"),
    to(&["PluginManager"], "callEvent", "server().message_plugin(name, channel, value) (no custom events yet)"),
    missing(&["PluginManager"], "disablePlugin", "No disabling one plugin yet: `/plugins reload`."),
    // The plugin.
    to(PLUGIN, "getConfig", "ctx.config()"),
    to(PLUGIN, "saveDefaultConfig", "ctx.save_default_config(include_str!(\"../config.yml\"))"),
    to(PLUGIN, "saveConfig", "ctx.save_config()"),
    to(PLUGIN, "reloadConfig", "ctx.reload_config()"),
    to(PLUGIN, "getDataFolder", "ctx.data_folder()"),
    to(PLUGIN, "getLogger", "ctx.logger()"),
    to(PLUGIN, "getServer", "ctx.server()"),
    to(PLUGIN, "getDescription", "ctx.description()"),
    to(PLUGIN, "getPluginMeta", "ctx.description()"),
    to(PLUGIN, "getName", "ctx.description().name"),
    to(PLUGIN, "getCommand", "(ctx.set_executor(name, ...) / ctx.set_tab_completer(name, ...))"),
    missing(PLUGIN, "saveResource", "Embed the file with include_str! and write it into ctx.data_folder() if missing."),
    missing(PLUGIN, "getResource", "Embed the file with include_str!/include_bytes!."),
    to(&["PluginCommand", "Command"], "setExecutor", "ctx.set_executor(name, |plugin, ctx, sender, label, args| ...)"),
    to(
        &["PluginCommand", "Command"],
        "setTabCompleter",
        "ctx.set_tab_completer(name, |plugin, ctx, sender, label, args| ...)",
    ),
    to(&["PluginCommand", "Command"], "getName", "the command's name (the executor's `label` is what was typed)"),
    // Config.
    to(CONFIG, "getString", "ctx.config().get_string(path) / string_or(path, default)"),
    to(CONFIG, "getInt", "ctx.config().int_or(path, 0)"),
    to(CONFIG, "getLong", "ctx.config().int_or(path, 0)"),
    to(CONFIG, "getDouble", "ctx.config().f64_or(path, 0.0)"),
    to(CONFIG, "getBoolean", "ctx.config().bool_or(path, false)"),
    to(CONFIG, "getStringList", "ctx.config().get_strings(path)"),
    to(CONFIG, "getList", "ctx.config().get(path)"),
    to(CONFIG, "get", "ctx.config().get(path)"),
    to(CONFIG, "set", "ctx.config().set(path, value)"),
    to(CONFIG, "contains", "ctx.config().contains(path)"),
    to(CONFIG, "isSet", "ctx.config().contains(path)"),
    to(CONFIG, "getKeys", "ctx.config().keys(path, deep)"),
    to(CONFIG, "getConfigurationSection", "ctx.config().keys(\"section\", false), then \"section.key\" paths"),
    to(CONFIG, "options", "(defaults are copied by ctx.save_default_config)"),
    to(CONFIG, "loadConfiguration", "Config::load(path)"),
    to(CONFIG, "save", "config.save()"),
    // Scheduling.
    to(SCHEDULER, "runTask", "ctx.run_task(|plugin, ctx| ...)"),
    to(SCHEDULER, "runTaskLater", "ctx.run_later(delay_ticks, |plugin, ctx| ...)"),
    to(SCHEDULER, "runTaskTimer", "ctx.run_timer(delay_ticks, period_ticks, |plugin, ctx| ...)"),
    to(SCHEDULER, "scheduleSyncDelayedTask", "ctx.run_later(delay_ticks, |plugin, ctx| ...)"),
    to(SCHEDULER, "scheduleSyncRepeatingTask", "ctx.run_timer(delay_ticks, period_ticks, |plugin, ctx| ...)"),
    to(SCHEDULER, "runTaskAsynchronously", "ctx.run_async(|sync| ... sync.run(|plugin, ctx| ...))"),
    to(SCHEDULER, "runTaskLaterAsynchronously", "ctx.run_async with a sleep, then sync.run(...)"),
    to(SCHEDULER, "runTaskTimerAsynchronously", "ctx.run_timer, with the slow part in ctx.run_async"),
    to(SCHEDULER, "cancelTask", "ctx.cancel_task(id)"),
    to(SCHEDULER, "cancelTasks", "ctx.cancel_all_tasks()"),
    to(SCHEDULER, "cancel", "ctx.cancel_task(id) (inside the task: ctx.current_task())"),
    to(&["BukkitTask", "ScheduledTask"], "cancel", "ctx.cancel_task(id)"),
    to(&["BukkitTask"], "getTaskId", "the TaskId ctx.run_timer returned"),
    missing(
        &["GlobalRegionScheduler", "RegionScheduler", "EntityScheduler", "AsyncScheduler"],
        "*",
        "Folia's schedulers: ctx.run_task / run_later / run_timer (one server thread here).",
    ),
    // Logging.
    to(LOGGER, "info", "ctx.logger().info(text)"),
    to(LOGGER, "warning", "ctx.logger().warning(text)"),
    to(LOGGER, "severe", "ctx.logger().severe(text)"),
    to(LOGGER, "fine", "ctx.logger().fine(text)"),
    to(LOGGER, "log", "ctx.logger().info / warning / severe"),
    // Colours.
    to(&["ChatColor"], "translateAlternateColorCodes", "color::translate('&', text)"),
    to(&["ChatColor"], "stripColor", "color::strip(text)"),
    to(&["ChatColor"], "getLastColors", "color::last_colors(text)"),
    to(&["ChatColor"], "toString", "ChatColor::Red.to_string() (or a § code in the text)"),
    missing(
        &["MiniMessage", "Component", "TextComponent", "LegacyComponentSerializer", "PlainTextComponentSerializer"],
        "*",
        "No Adventure components: plain text with § codes (color::translate('&', ...)).",
    ),
    missing(
        &["TextComponent", "ComponentBuilder", "ClickEvent", "HoverEvent"],
        "*",
        "No BungeeCord chat components: plain text with § codes; `/tellraw` with server().dispatch_command for clicks.",
    ),
    // Worlds, blocks and places.
    to(WORLD, "getName", "world.name"),
    to(WORLD, "getPlayers", "world.players()"),
    to(WORLD, "getBlockAt", "world.block_at(x, y, z)"),
    to(WORLD, "spawnEntity", "world.spawn_entity(\"minecraft:zombie\", &location)"),
    to(WORLD, "spawnParticle", "world.spawn_particle(...)"),
    to(WORLD, "strikeLightning", "world.strike_lightning(&location)"),
    to(WORLD, "strikeLightningEffect", "world.strike_lightning(&location)"),
    to(WORLD, "createExplosion", "world.create_explosion(&location, power)"),
    to(WORLD, "setTime", "world.set_time(ticks)"),
    to(WORLD, "setFullTime", "world.set_time(ticks)"),
    to(WORLD, "setStorm", "world.set_weather(\"rain\", seconds)"),
    to(WORLD, "setThundering", "world.set_weather(\"thunder\", seconds)"),
    to(WORLD, "getEntities", "world.entities(None)"),
    to(WORLD, "getLivingEntities", "world.entities(None)"),
    to(WORLD, "getEntitiesByClass", "world.entities(Some(\"minecraft:zombie\"))"),
    missing(WORLD, "getSpawnLocation", "No world spawn yet: keep one in config.yml."),
    missing(WORLD, "getHighestBlockAt", "No heightmap yet: read down with world.block_at."),
    missing(WORLD, "getTime", "No time read yet."),
    missing(
        WORLD,
        "dropItem",
        "No item entities yet: world.spawn_entity or `/summon item` with server().dispatch_command.",
    ),
    missing(WORLD, "dropItemNaturally", "No item entities yet: `/summon item` with server().dispatch_command."),
    missing(WORLD, "playSound", "Per player only: player.play_sound for each of world.players()."),
    to(BLOCK, "getType", "block.kind (\"minecraft:stone\"), or block.is(\"stone\")"),
    to(BLOCK, "setType", "block.pos.set_type(\"minecraft:stone\")"),
    to(BLOCK, "getLocation", "block.pos.location()"),
    to(BLOCK, "getWorld", "block.pos.location().world()"),
    to(BLOCK, "getX", "block.pos.x"),
    to(BLOCK, "getY", "block.pos.y"),
    to(BLOCK, "getZ", "block.pos.z"),
    to(BLOCK, "getRelative", "block.pos.offset(dx, dy, dz)"),
    to(BLOCK, "getBlockData", "block.properties / block.state_string()"),
    to(BLOCK, "isEmpty", "block.is_air()"),
    missing(BLOCK, "getState", "No block entity data yet (chests, signs, spawners)."),
    missing(
        BLOCK,
        "breakNaturally",
        "block.pos.set_type(\"minecraft:air\") (no drops), or `/setblock ... destroy` with server().dispatch_command.",
    ),
    missing(BLOCK, "getDrops", "No drops yet."),
    to(LOCATION, "<init>", "Location::new(\"minecraft:overworld\", x, y, z).with_rotation(yaw, pitch)"),
    to(LOCATION, "getX", "location.x"),
    to(LOCATION, "getY", "location.y"),
    to(LOCATION, "getZ", "location.z"),
    to(LOCATION, "getYaw", "location.yaw"),
    to(LOCATION, "getPitch", "location.pitch"),
    to(LOCATION, "getBlockX", "location.block_x()"),
    to(LOCATION, "getBlockY", "location.block_y()"),
    to(LOCATION, "getBlockZ", "location.block_z()"),
    to(LOCATION, "getWorld", "location.world()"),
    to(LOCATION, "getBlock", "location.block() (a BlockPos; .block() reads it)"),
    to(LOCATION, "distance", "location.distance(&other)"),
    to(LOCATION, "distanceSquared", "location.distance_squared(&other)"),
    to(LOCATION, "add", "location.add(dx, dy, dz)"),
    to(LOCATION, "clone", "location.clone()"),
    // Items.
    to(ITEM, "<init>", "ItemStack::new(\"minecraft:diamond\", amount)"),
    to(ITEM, "getType", "item.kind (\"minecraft:diamond\"), or item.is(\"diamond\")"),
    to(ITEM, "getAmount", "item.amount"),
    to(ITEM, "setAmount", "item.amount = n"),
    to(ITEM, "getItemMeta", "(name and lore are on the ItemStack itself)"),
    to(ITEM, "setItemMeta", "(name and lore are on the ItemStack itself)"),
    to(ITEM, "setDisplayName", "item.named(\"§6Name\")"),
    to(ITEM, "displayName", "item.named(\"§6Name\")"),
    to(ITEM, "setLore", "item.lore_line(\"§7line\") for each line"),
    to(ITEM, "lore", "item.lore_line(\"§7line\") for each line"),
    to(ITEM, "addItem", "player.give_item(&item) (or player.give(\"minecraft:x\", n))"),
    to(ITEM, "removeItem", "player.take(\"minecraft:x\", n)"),
    to(ITEM, "contains", "player.count(\"minecraft:x\") >= n"),
    to(ITEM, "containsAtLeast", "player.count(\"minecraft:x\") >= n"),
    to(ITEM, "setItem", "player.set_slot(slot, Some(&item)), or menu.set(slot, Some(item))"),
    to(ITEM, "getItem", "player.inventory() / menu.get(slot)"),
    to(ITEM, "getItemInMainHand", "player.item_in_hand()"),
    to(ITEM, "setItemInMainHand", "player.set_equipment(\"weapon.mainhand\", Some(&item))"),
    to(ITEM, "setHelmet", "player.set_equipment(\"armor.head\", Some(&item))"),
    to(ITEM, "setChestplate", "player.set_equipment(\"armor.chest\", Some(&item))"),
    to(ITEM, "setLeggings", "player.set_equipment(\"armor.legs\", Some(&item))"),
    to(ITEM, "setBoots", "player.set_equipment(\"armor.feet\", Some(&item))"),
    to(ITEM, "clear", "player.clear_inventory()"),
    to(ITEM, "getContents", "player.inventory()"),
    missing(
        ITEM,
        "addEnchantment",
        "No enchantments on ItemStack yet: `/give` with components through server().dispatch_command.",
    ),
    missing(ITEM, "addUnsafeEnchantment", "No enchantments on ItemStack yet."),
    missing(ITEM, "getPersistentDataContainer", "No persistent data on items yet."),
    missing(
        ITEM,
        "setCustomModelData",
        "No item components yet: `/give` with components through server().dispatch_command.",
    ),
    // Scoreboards and boss bars.
    to(&["ScoreboardManager", "Scoreboard"], "registerNewObjective", "Objective::create(name, criteria, title)"),
    to(&["Scoreboard"], "getObjective", "Objective::named(name)"),
    to(&["Scoreboard"], "registerNewTeam", "Team::create(name)"),
    to(&["Scoreboard"], "getTeam", "Team::named(name)"),
    to(&["Objective"], "setDisplaySlot", "objective.show(DisplaySlot::Sidebar)"),
    to(&["Objective"], "getScore", "objective.set(holder, score) / add(holder, n)"),
    to(&["Score"], "setScore", "objective.set(holder, score)"),
    to(&["Team"], "addEntry", "team.add_player(&player)"),
    to(&["Team"], "setPrefix", "team.set_prefix(text)"),
    to(&["Team"], "setSuffix", "team.set_suffix(text)"),
    to(&["BossBar"], "addPlayer", "bar.show_to(&[player])"),
    to(&["BossBar"], "setProgress", "bar.set_progress(p)"),
    to(&["BossBar"], "setTitle", "bar.set_title(text)"),
    to(&["BossBar"], "setVisible", "bar.set_visible(on)"),
    to(&["BossBar"], "removeAll", "bar.remove()"),
    missing(&["Player"], "setScoreboard", "One scoreboard for all here (vanilla's): Objective / Team."),
    // Common in real plugins.
    to(SERVER, "getServer", "server()"),
    to(SERVER, "getOnlineMode", "true (Horizon checks accounts unless online-mode is off in server.properties)"),
    to(SERVER, "isPrimaryThread", "true, except inside ctx.run_async"),
    to(SERVER, "isOwnedByCurrentRegion", "true (one server thread, no Folia regions)"),
    to(SERVER, "isStopping", "false while the plugin runs (on_disable comes as the server stops)"),
    to(SERVER, "getGlobalRegionScheduler", "ctx.run_task / run_later / run_timer (Folia's: one server thread here)"),
    to(SERVER, "getRegionScheduler", "ctx.run_task / run_later / run_timer (Folia's: one server thread here)"),
    to(SERVER, "getAsyncScheduler", "ctx.run_async(|sync| ...)"),
    to(SERVER, "getPluginCommand", "(ctx.set_executor(name, ...))"),
    to(SERVER, "getIp", "(server.properties)"),
    to(SERVER, "getPort", "(server.properties)"),
    to(SERVER, "getPluginsFolder", "ctx.data_folder().parent()"),
    to(SERVER, "getWorldContainer", "(the server folder)"),
    to(SERVER, "getCommandMap", "ctx.command(CommandSpec::new(name), ...)"),
    missing(SERVER, "getUnsafe", "Bukkit's unsafe values are CraftBukkit's: no equivalent."),
    missing(
        SERVER,
        "getMessenger",
        "No plugin channels to clients yet; between plugins: server().message_plugin and ctx.on_message.",
    ),
    missing(SERVER, "getTag", "No tags yet: list the block ids."),
    missing(SERVER, "getRegistry", "No registries: ids are strings (\"minecraft:diamond\")."),
    missing(SERVER, "getViewDistance", "No server settings yet."),
    missing(SERVER, "getServerTickManager", "No tick rate control yet."),
    missing(SERVER, "getTPS", "No TPS yet: ServerTickStartEvent / ServerTickEndEvent can time ticks."),
    to(&["PluginManager"], "registerEvent", "ctx.listen::<E>(priority, ignore_cancelled, |plugin, ctx, e| ...)"),
    to(&["PluginManager"], "getPlugins", "server().plugins()"),
    to(PLUGIN, "isEnabled", "true while the plugin runs"),
    to(PLUGIN, "getPluginLoader", "(no loaders: ctx is the plugin's handle)"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getVersion", "ctx.description().version"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getName", "ctx.description().name"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getDescription", "ctx.description().description"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getAuthors", "ctx.description().authors"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getWebsite", "ctx.description().website"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getFullName", "format!(\"{} v{}\", d.name, d.version)"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getDepend", "ctx.description().depend"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getSoftDepend", "ctx.description().soft_depend"),
    to(&["PluginDescriptionFile", "PluginMeta"], "getCommands", "ctx.description().commands"),
    to(LOGGER, "getLogger", "ctx.logger()"),
    to(LOGGER, "isLoggable", "true"),
    to(LOGGER, "config", "ctx.logger().fine(text)"),
    to(LOGGER, "finer", "ctx.logger().fine(text)"),
    to(LOGGER, "finest", "ctx.logger().fine(text)"),
    to(CONFIG, "addDefault", "(put the default in config.yml; ctx.save_default_config copies new keys)"),
    to(CONFIG, "addDefaults", "(put the defaults in config.yml; ctx.save_default_config copies new keys)"),
    to(
        &["YamlConfigurationOptions", "FileConfigurationOptions", "ConfigurationOptions"],
        "*",
        "(defaults come from config.yml through ctx.save_default_config; comments stay in the file)",
    ),
    to(CONFIG, "getSection", "ctx.config().keys(path, false)"),
    to(CONFIG, "isString", "ctx.config().get_string(path).is_some()"),
    to(CONFIG, "isInt", "ctx.config().get_int(path).is_some()"),
    to(CONFIG, "isBoolean", "ctx.config().get_bool(path).is_some()"),
    to(CONFIG, "isList", "matches!(ctx.config().get(path), Some(Value::List(_)))"),
    to(CONFIG, "isConfigurationSection", "!ctx.config().keys(path, false).is_empty()"),
    to(CONFIG, "getIntegerList", "ctx.config().get(path) (a Value::List)"),
    to(CONFIG, "getCurrentPath", "the path string the plugin built"),
    to(CONFIG, "loadFromString", "Config::from_yaml(path, text)"),
    to(CONFIG, "saveToString", "config.to_yaml()"),
    to(CONFIG, "load", "Config::load(path)"),
    to(CONFIG, "getValues", "ctx.config().values()"),
    to(CONFIG, "createSection", "ctx.config().set(\"section.key\", value)"),
    to(
        WORLD,
        "getEnvironment",
        "location.world: \"minecraft:overworld\", \"minecraft:the_nether\" or \"minecraft:the_end\"",
    ),
    to(WORLD, "getMinHeight", "-64 (the overworld; 0 in the nether and the end)"),
    to(WORLD, "getMaxHeight", "320 (the overworld; 256 in the nether and the end)"),
    to(WORLD, "getUID", "world.name (worlds are named, not numbered)"),
    to(WORLD, "getKey", "world.name (\"minecraft:overworld\")"),
    missing(WORLD, "getChunkAt", "No chunks yet: read blocks with world.block_at or world.region(min, max)."),
    missing(WORLD, "getChunkAtAsync", "No chunks yet: read blocks with world.block_at or world.region(min, max)."),
    missing(WORLD, "isChunkLoaded", "No chunks yet."),
    missing(WORLD, "isChunkGenerated", "No chunks yet."),
    missing(WORLD, "getLoadedChunks", "No chunks yet."),
    missing(WORLD, "loadChunk", "No chunks yet: `/forceload` with server().dispatch_command."),
    missing(WORLD, "getWorldBorder", "No world border yet: `/worldborder` with server().dispatch_command."),
    missing(WORLD, "getGameRuleValue", "No game rules yet: `/gamerule` with server().dispatch_command."),
    missing(WORLD, "setGameRule", "No game rules yet: `/gamerule` with server().dispatch_command."),
    missing(WORLD, "getBiome", "No biomes yet."),
    missing(WORLD, "getSeed", "No seed yet."),
    missing(WORLD, "save", "server().save_all() saves every world."),
    to(&["Material"], "isAir", "block.is_air(), or kind == \"minecraft:air\""),
    to(&["Material"], "getKey", "the id itself: \"minecraft:diamond\""),
    to(&["Material"], "name", "the id: \"minecraft:diamond\" (lower case)"),
    to(&["Material"], "valueOf", "\"minecraft:\" + name.to_lowercase()"),
    to(&["Material"], "matchMaterial", "server().block_exists(id) to check an id"),
    to(&["Material"], "values", "(no list of ids: server().block_exists(id) checks one)"),
    missing(&["Material"], "isSolid", "No material properties yet: list the ids that count."),
    missing(&["Material"], "isBlock", "No material properties yet: server().block_exists(id) tells blocks."),
    missing(&["Material"], "isItem", "No material properties yet."),
    missing(&["Material"], "createBlockData", "Block states are strings: \"minecraft:oak_stairs[facing=east]\"."),
    missing(&["Material"], "isOccluding", "No material properties yet."),
    missing(&["Material"], "isFlammable", "No material properties yet."),
    missing(&["Material"], "getMaxStackSize", "No item properties yet: 64 for most, 16 or 1 for some."),
    to(&["NamespacedKey", "Key", "Keyed"], "*", "ids are strings: \"namespace:value\""),
    to(&["Registry", "RegistryAccess"], "*", "ids are strings (\"minecraft:diamond\"); no registries to read"),
    missing(&["Tag"], "*", "No tags yet: list the ids."),
    to(PLAYER, "spigot", "(Spigot's extras: plain text with player.send_message)"),
    to(&["Spigot"], "sendMessage", "player.send_message(text) (plain text with § codes)"),
    missing(&["Spigot"], "*", "Spigot's extras aren't here."),
    to(PLAYER, "getPlayer", "the player itself"),
    to(PLAYER, "hasPlayedBefore", "keep the UUIDs seen in a file (no offline player data)"),
    to(PLAYER, "getEyeLocation", "player.location.add(0.0, 1.62, 0.0)"),
    to(PLAYER, "getEntityId", "(Entity.id for mobs; players by uuid)"),
    to(PLAYER, "getType", "\"minecraft:player\""),
    missing(PLAYER, "isSneaking", "No sneaking yet."),
    missing(PLAYER, "isSprinting", "No sprinting yet."),
    missing(PLAYER, "isFlying", "No flight yet."),
    missing(PLAYER, "isOnGround", "No on-ground flag yet: read the block under player.location."),
    missing(PLAYER, "sendPluginMessage", "No plugin channels to clients yet."),
    missing(PLAYER, "getListeningPluginChannels", "No plugin channels to clients yet."),
    missing(PLAYER, "getPing", "No ping yet."),
    missing(PLAYER, "getLocale", "No client locale yet."),
    missing(PLAYER, "locale", "No client locale yet."),
    missing(PLAYER, "getClientBrandName", "No client brand yet."),
    missing(PLAYER, "getProtocolVersion", "One version: the server's (26.3)."),
    missing(PLAYER, "getFoodLevel", "No hunger yet."),
    missing(PLAYER, "getLevel", "No XP read yet."),
    missing(PLAYER, "getExp", "No XP read yet."),
    missing(PLAYER, "getActivePotionEffects", "No effects read yet."),
    missing(PLAYER, "hasPotionEffect", "No effects read yet."),
    missing(PLAYER, "getAttribute", "No attributes yet: `/attribute` with server().dispatch_command."),
    missing(PLAYER, "getBedSpawnLocation", "No respawn point yet."),
    missing(PLAYER, "getRespawnLocation", "No respawn point yet."),
    missing(PLAYER, "getVelocity", "No velocity yet."),
    missing(PLAYER, "getVehicle", "No vehicles yet."),
    missing(PLAYER, "getNearbyEntities", "world.entities(None), filtered by distance to player.location."),
    missing(PLAYER, "getOpenInventory", "Only the menus the plugin opens (player.open_menu)."),
    missing(PLAYER, "updateInventory", "Changes reach the client at once."),
    missing(PLAYER, "sendBlockChange", "No fake blocks yet."),
    missing(PLAYER, "setResourcePack", "No resource packs yet."),
    missing(PLAYER, "setCompassTarget", "No compass target yet."),
    missing(
        PLAYER,
        "setInvulnerable",
        "No invulnerability yet: cancel PrePlayerAttackEntityEvent, or `/effect resistance`.",
    ),
    missing(PLAYER, "damage", "No damage yet: `/damage` with server().dispatch_command."),
    missing(PLAYER, "setFireTicks", "No fire yet."),
    missing(PLAYER, "setOp", "No op changes yet: `/op` and `/deop` with server().dispatch_command."),
    missing(PLAYER, "setWhitelisted", "server().whitelist_add(name) / whitelist_remove(name)."),
    missing(PLAYER, "isBanned", "No ban list read yet."),
    missing(PLAYER, "ban", "server().ban(name, reason)."),
    missing(PLAYER, "addAttachment", "No permission attachments: plugins/permissions.yml holds groups and players."),
    missing(PLAYER, "recalculatePermissions", "Permissions are read again on /plugins reload."),
    missing(PLAYER, "getEffectivePermissions", "Only has_permission(node)."),
    missing(PLAYER, "setCollidable", "No collision control yet: a Team rule (`/team modify collisionRule`)."),
    missing(PLAYER, "setGlowing", "No glowing yet: `/effect give <name> glowing`."),
    missing(PLAYER, "setCustomName", "No entity names yet."),
    missing(PLAYER, "remove", "Only world.kill_entities(filter, &location) for mobs."),
    to(ITEM, "getHelmet", "player.inventory() (armour slots)"),
    to(ITEM, "getChestplate", "player.inventory() (armour slots)"),
    to(ITEM, "getLeggings", "player.inventory() (armour slots)"),
    to(ITEM, "getBoots", "player.inventory() (armour slots)"),
    to(ITEM, "getArmorContents", "player.inventory() (armour slots)"),
    to(ITEM, "getItemInOffHand", "player.inventory() (the off hand slot)"),
    to(ITEM, "setItemInOffHand", "player.set_equipment(\"weapon.offhand\", Some(&item))"),
    to(ITEM, "getHeldItemSlot", "(the selected slot isn't sent yet: player.item_in_hand())"),
    to(ITEM, "getSize", "menu.size()"),
    to(ITEM, "firstEmpty", "the first None in player.inventory()"),
    to(ITEM, "hasItemMeta", "item.name.is_some() || !item.lore.is_empty()"),
    to(ITEM, "hasDisplayName", "item.name.is_some()"),
    to(ITEM, "getDisplayName", "item.name"),
    to(ITEM, "hasLore", "!item.lore.is_empty()"),
    to(ITEM, "getLore", "item.lore"),
    to(ITEM, "isSimilar", "a.kind == b.kind && a.name == b.name && a.lore == b.lore"),
    to(ITEM, "clone", "item.clone()"),
    to(ITEM, "getMaxStackSize", "64 for most items"),
    missing(ITEM, "getHolder", "Menus are the plugin's own: tell them by the window id player.open_menu gave."),
    missing(ITEM, "getViewers", "Menus are the plugin's own: keep who has one open."),
    missing(ITEM, "getEnchantments", "No enchantments on ItemStack yet."),
    missing(ITEM, "addItemFlags", "No item flags yet."),
    missing(ITEM, "setUnbreakable", "No item components yet."),
    missing(ITEM, "getDurability", "No damage on ItemStack yet."),
    missing(ITEM, "setDurability", "No damage on ItemStack yet."),
    missing(BLOCK, "update", "No block entity data yet: block.pos.set_type(state) writes the block."),
    missing(BLOCK, "getChunk", "No chunks yet."),
    missing(BLOCK, "isLiquid", "kind is \"minecraft:water\" or \"minecraft:lava\"."),
    missing(BLOCK, "getLightLevel", "No light yet."),
    missing(BLOCK, "getBiome", "No biomes yet."),
    to(BLOCK, "isPassable", "block.is_air() (or list the ids that count)"),
    to(BLOCK, "setBlockData", "block.pos.set_type(\"minecraft:oak_stairs[facing=east]\")"),
    to(BLOCK, "getData", "block.properties"),
    to(&["BlockData"], "getAsString", "block.state_string()"),
    to(&["BlockData"], "getMaterial", "block.kind"),
    missing(
        &["BlockData", "Directional", "Ageable", "Waterlogged", "Bisected", "Levelled", "Openable", "Powerable"],
        "*",
        "Block states are strings: block.property(\"facing\"), and set_type(\"id[key=value]\").",
    ),
    to(LOCATION, "getChunk", "(no chunks: block_x() >> 4, block_z() >> 4)"),
    to(LOCATION, "toVector", "(location.x, location.y, location.z)"),
    to(LOCATION, "getDirection", "location.facing() (a BlockFace; yaw and pitch for the exact vector)"),
    to(LOCATION, "setX", "location.x = x"),
    to(LOCATION, "setY", "location.y = y"),
    to(LOCATION, "setZ", "location.z = z"),
    to(LOCATION, "setYaw", "location.yaw = yaw"),
    to(LOCATION, "setPitch", "location.pitch = pitch"),
    to(LOCATION, "setWorld", "location.world = name"),
    to(LOCATION, "subtract", "location.add(-dx, -dy, -dz)"),
    to(LOCATION, "toBlockLocation", "location.block().location()"),
    to(&["Vector"], "*", "(x, y, z) as f64s; Location::add for offsets"),
    to(&["ChatColor"], "of", "a § code (no hex colours yet: the 16 of ChatColor)"),
    to(&["ChatColor"], "name", "format!(\"{:?}\", color)"),
    to(&["ChatColor"], "values", "ChatColor::ALL"),
    to(&["ChatColor"], "getChar", "color.code()"),
    to(&["ChatColor"], "getByChar", "ChatColor::by_code(c)"),
    to(&["Entity", "LivingEntity"], "getEntityId", "entity.id"),
    to(&["Entity", "LivingEntity"], "getType", "entity.kind (\"minecraft:zombie\")"),
    to(&["EntityType"], "*", "ids are strings: \"minecraft:zombie\""),
    to(&["GameMode"], "*", "GameMode::Survival / Creative / Adventure / Spectator"),
    to(&["Sound"], "*", "ids are strings: \"minecraft:entity.player.levelup\""),
    to(&["Particle"], "*", "ids are strings: \"minecraft:flame\""),
    to(
        &["PotionEffectType", "PotionEffect"],
        "*",
        "player.add_effect(\"minecraft:speed\", seconds, amplifier, particles)",
    ),
    to(&["Enchantment"], "*", "ids are strings: \"minecraft:sharpness\" (no enchanting through the API yet)"),
    to(&["EventPriority"], "*", "EventPriority::Lowest ... Monitor"),
    to(&["HandlerList"], "unregisterAll", "(listeners last until the plugin is disabled or reloaded)"),
    to(&["HandlerList"], "*", "(events need no handler lists here)"),
    to(&["Event"], "callEvent", "server().message_plugin(name, channel, value) (no custom events yet)"),
    to(&["Event"], "getEventName", "the event type's name"),
    to(&["Event"], "isAsynchronous", "false (every event runs on the server thread)"),
    missing(
        &["PersistentDataContainer", "PersistentDataType", "PersistentDataHolder"],
        "*",
        "No persistent data yet: keep it in a file in ctx.data_folder().",
    ),
    missing(
        &["MetadataValue", "FixedMetadataValue", "Metadatable"],
        "*",
        "No metadata: keep a HashMap in the plugin's struct.",
    ),
    missing(
        &[
            "BukkitObjectInputStream",
            "BukkitObjectOutputStream",
            "ConfigurationSerialization",
            "ConfigurationSerializable",
        ],
        "*",
        "No Java serialization: write the fields to YAML (Config) or JSON.",
    ),
    missing(
        &["Recipe", "ShapedRecipe", "ShapelessRecipe", "FurnaceRecipe"],
        "*",
        "No recipes yet: a data pack can add them.",
    ),
    missing(&["Chunk"], "*", "No chunks yet."),
    missing(&["WorldBorder"], "*", "No world border yet: `/worldborder` with server().dispatch_command."),
    missing(&["WorldCreator"], "*", "No making worlds yet: the server has its world (overworld, nether, end)."),
    missing(
        &["Messenger", "PluginMessageListener"],
        "*",
        "No plugin channels to clients yet; between plugins: server().message_plugin and ctx.on_message.",
    ),
    missing(
        &["Permission", "PermissionAttachment", "PermissionAttachmentInfo"],
        "*",
        "Permissions are declared in plugin.yml and granted in plugins/permissions.yml.",
    ),
    missing(
        &["ProtocolManager", "PacketContainer", "PacketAdapter"],
        "*",
        "ProtocolLib's packets: no packets reach plugins.",
    ),
    missing(&["Economy"], "*", "Vault's economy: no economy plugin yet; between plugins: server().message_plugin."),
    missing(&["PlaceholderAPI", "PlaceholderExpansion"], "*", "PlaceholderAPI: no placeholders plugin yet."),
    // Events, by their getters.
    to(EVENT, "getPlayer", "e.player"),
    to(EVENT, "getWhoClicked", "e.player"),
    to(EVENT, "getEntity", "e.entity (e.player in player events)"),
    to(EVENT, "getBlock", "e.block"),
    to(EVENT, "getClickedBlock", "e.block"),
    to(EVENT, "getBlockPlaced", "e.block"),
    to(EVENT, "getBlockAgainst", "e.against"),
    to(EVENT, "getBlockFace", "e.face"),
    to(EVENT, "getItem", "e.item"),
    to(EVENT, "getItemInHand", "e.item"),
    to(EVENT, "getAction", "e.action (\"right_click_block\", \"right_click_air\", \"left_click_block\")"),
    to(EVENT, "getFrom", "e.from"),
    to(EVENT, "getTo", "e.to"),
    to(EVENT, "getMessage", "e.message"),
    to(EVENT, "setMessage", "e.message = text"),
    to(EVENT, "message", "e.message"),
    to(EVENT, "getFormat", "e.format"),
    to(EVENT, "setFormat", "e.format = text"),
    to(EVENT, "renderer", "e.format = \"<%1$s> %2$s\" (a format string, not a renderer)"),
    to(EVENT, "setJoinMessage", "e.join_message = Some(text) / None"),
    to(EVENT, "joinMessage", "e.join_message = Some(text) / None"),
    to(EVENT, "getJoinMessage", "e.join_message"),
    to(EVENT, "setQuitMessage", "e.quit_message = Some(text) / None"),
    to(EVENT, "quitMessage", "e.quit_message = Some(text) / None"),
    to(EVENT, "getQuitMessage", "e.quit_message"),
    to(EVENT, "setCancelled", "e.set_cancelled(true)"),
    to(EVENT, "isCancelled", "e.is_cancelled()"),
    to(EVENT, "blockList", "e.blocks"),
    to(EVENT, "getLocation", "e.location"),
    to(EVENT, "getToBlock", "e.to"),
    to(EVENT, "getSource", "e.source"),
    to(EVENT, "getDirection", "e.direction"),
    to(EVENT, "getBlocks", "e.blocks"),
    to(EVENT, "getRawSlot", "e.slot"),
    to(EVENT, "getSlot", "e.slot"),
    to(EVENT, "getNewGameMode", "e.player.game_mode"),
    to(EVENT, "getPlugin", "e.plugin"),
    to(EVENT, "getDamager", "e.player (PrePlayerAttackEntityEvent: the attacker)"),
    to(EVENT, "getAttacked", "e.entity"),
    to(EVENT, "getEntityType", "e.entity.kind"),
    to(EVENT, "getView", "(menus are the plugin's own: keep their window id from player.open_menu)"),
    to(EVENT, "getInventory", "(menus are the plugin's own: keep their window id from player.open_menu)"),
    to(EVENT, "getCurrentItem", "the item the plugin put at e.slot in its Menu"),
    missing(EVENT, "getDamage", "No damage amount yet."),
    missing(EVENT, "setDamage", "No damage amount yet: cancel the blow, or `/effect` with server().dispatch_command."),
    missing(EVENT, "getDrops", "No drops yet."),
    missing(EVENT, "setDropItems", "No drops yet."),
    missing(EVENT, "setExpToDrop", "No XP drops yet."),
    missing(EVENT, "getDeathMessage", "No death message yet."),
    missing(EVENT, "setDeathMessage", "No death message yet."),
    missing(EVENT, "deathMessage", "No death message yet."),
    missing(EVENT, "setRespawnLocation", "No respawn location yet: teleport the player from ctx.run_task."),
    missing(EVENT, "setKeepInventory", "No keep inventory yet: the gamerule `keepInventory`."),
    missing(EVENT, "getCause", "No cause yet."),
    missing(EVENT, "getSpawnReason", "No spawn reason yet."),
    missing(EVENT, "getClick", "e.mode and e.button (0 pick up, 1 shift click, 2 number key...)."),
    missing(EVENT, "getHand", "No hand yet: the main hand."),
    missing(EVENT, "setUseItemInHand", "Only cancelling the whole click (e.set_cancelled)."),
    missing(EVENT, "setUseInteractedBlock", "Only cancelling the whole click (e.set_cancelled)."),
];

/// What a call to `owner.name` (owner an internal name) becomes, if it is Bukkit's or Paper's.
pub fn call(owner: &str, name: &str) -> Option<Port> {
    let simple = simple_name(owner);
    let owner_matches =
        |owners: &[&str]| owners.iter().any(|o| *o == simple || (*o == "*Event" && simple.ends_with("Event")));
    // Exact names first, then a class's catch-all.
    let found = CALLS
        .iter()
        .find(|c| c.name == name && owner_matches(c.owners))
        .or_else(|| CALLS.iter().find(|c| c.name == "*" && owner_matches(c.owners)))
        .map(|c| c.port);
    if found.is_some() {
        return found;
    }
    if name == "equals" {
        return Some(Port::To("=="));
    }
    if owner.starts_with("net/kyori/adventure/") {
        return Some(Port::Missing("No Adventure components: plain text with § codes (color::translate('&', ...))."));
    }
    if owner.starts_with("net/md_5/bungee/api/chat/") {
        return Some(Port::Missing(
            "No BungeeCord chat components: plain text with § codes; `/tellraw` through server().dispatch_command for clicks.",
        ));
    }
    if owner.starts_with("io/papermc/paper/threadedregions/") {
        return Some(Port::To("ctx.run_task / run_later / run_timer (Folia's: one server thread here)"));
    }
    if owner.starts_with("io/papermc/paper/command/brigadier/") || owner.starts_with("com/mojang/brigadier/") {
        return Some(Port::To("ctx.command(CommandSpec::new(name), ...) and ctx.set_tab_completer"));
    }
    // Whole packages whose part of the game the API doesn't reach yet.
    if owner.starts_with("org/bukkit/inventory/meta/") {
        return Some(Port::Missing(
            "Only a name and lore on ItemStack yet: other components through `/give` with server().dispatch_command.",
        ));
    }
    if owner.starts_with("org/bukkit/entity/") {
        return Some(Port::Missing(
            "Mobs by handle aren't there yet: World::entities lists them, kill_entities removes them; vanilla commands (`/data`, `/summon`) for the rest.",
        ));
    }
    if owner.starts_with("org/bukkit/block/") {
        return Some(Port::Missing(
            "No block entity data or block properties beyond the state string yet (block.property, set_type with a state).",
        ));
    }
    if owner.starts_with("org/bukkit/inventory/") {
        return Some(Port::Missing("Only the player's inventory and chest menus yet."));
    }
    if owner.starts_with("org/bukkit/craftbukkit/")
        || owner.starts_with("net/minecraft/")
        || owner.starts_with("com/mojang/")
    {
        return Some(Port::Missing(
            "The server's internals (CraftBukkit, Minecraft's code): no equivalent, Horizon is another server.",
        ));
    }
    None
}

/// Whether `owner` (an internal name) is part of the server's API: Bukkit, Paper, Spigot, Adventure,
/// BungeeCord's chat, Java's logging as Bukkit hands it out.
pub fn is_api(owner: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "org/bukkit/",
        "io/papermc/paper/",
        "com/destroystokyo/paper/",
        "org/spigotmc/",
        "net/kyori/adventure/",
        "net/md_5/bungee/api/",
    ];
    PREFIXES.iter().any(|p| owner.starts_with(p))
        || owner == "java/util/logging/Logger"
        || owner.starts_with("net/minecraft/")
        || owner.starts_with("com/mojang/")
}

/// The last part of an internal name, without an outer class (`Player`, `Builder`).
pub fn simple_name(internal: &str) -> &str {
    let last = internal.rsplit('/').next().unwrap_or(internal);
    last.rsplit('$').next().unwrap_or(last)
}

/// A constant a plugin reads (`Material.DIAMOND`) as Horizon writes it.
pub fn constant(owner: &str, name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    Some(match simple_name(owner) {
        "Material" => format!("\"minecraft:{lower}\""),
        "EntityType" => format!("\"minecraft:{lower}\""),
        "Particle" => format!("\"minecraft:{lower}\""),
        "PotionEffectType" => format!("\"minecraft:{lower}\""),
        "Sound" => format!("\"minecraft:{}\" (check the sound's id)", lower.replacen('_', ".", 2)),
        "ChatColor" | "NamedTextColor" => format!("ChatColor::{}", camel(&lower)),
        "GameMode" => format!("GameMode::{}", camel(&lower)),
        "EventPriority" => format!("EventPriority::{}", camel(&lower)),
        "BarColor" => format!("BarColor::{}", camel(&lower)),
        "BarStyle" => format!("BarStyle::{}", camel(&lower)),
        "DisplaySlot" => format!("DisplaySlot::{}", camel(&lower)),
        "BlockFace" => format!("BlockFace::{}", camel(&lower)),
        "Action" => format!("\"{lower}\" (e.action)"),
        _ => return None,
    })
}

/// `dark_red` → `DarkRed`.
pub fn camel(snake: &str) -> String {
    snake
        .split(['_', '-', ' ', '.'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase()).unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_map_by_owner_family() {
        assert_eq!(call("org/bukkit/entity/Player", "sendMessage"), Some(Port::To("player.send_message(text)")));
        assert_eq!(
            call("org/bukkit/command/CommandSender", "sendMessage"),
            Some(Port::To("player.send_message(text)"))
        );
        assert_eq!(call("org/bukkit/event/player/PlayerJoinEvent", "getPlayer"), Some(Port::To("e.player")));
        assert!(matches!(call("org/bukkit/entity/Player", "setHealth"), Some(Port::Missing(_))));
        assert!(call("net/kyori/adventure/text/minimessage/MiniMessage", "deserialize").is_some());
        assert_eq!(call("org/bukkit/Server", "noSuchMethod"), None);
        assert!(matches!(call("org/bukkit/entity/Villager", "getProfession"), Some(Port::Missing(_))));
    }

    #[test]
    fn constants_read_as_horizon_writes_them() {
        assert_eq!(constant("org/bukkit/Material", "DIAMOND_SWORD").unwrap(), "\"minecraft:diamond_sword\"");
        assert_eq!(constant("org/bukkit/ChatColor", "DARK_RED").unwrap(), "ChatColor::DarkRed");
        assert_eq!(constant("org/bukkit/GameMode", "CREATIVE").unwrap(), "GameMode::Creative");
    }

    /// Names each type, so the test stops compiling when the API renames or drops an event.
    macro_rules! api_events {
        ($($t:ident),* $(,)?) => {{
            $( let _ = <horizon_plugin_api::prelude::$t as horizon_plugin_api::prelude::Event>::NAME; )*
            [$(stringify!($t)),*]
        }};
    }

    #[test]
    fn every_mapped_event_exists_in_the_api() {
        let known = api_events!(
            PlayerJoinEvent,
            PlayerQuitEvent,
            PlayerMoveEvent,
            PlayerChangedWorldEvent,
            PlayerDeathEvent,
            PlayerRespawnEvent,
            PlayerGameModeChangeEvent,
            AsyncPlayerChatEvent,
            PlayerInteractEvent,
            BlockBreakEvent,
            BlockPlaceEvent,
            PlayerBucketFillEvent,
            PlayerBucketEmptyEvent,
            PrePlayerAttackEntityEvent,
            EntitySpawnEvent,
            EntityDeathEvent,
            EntityExplodeEvent,
            BlockFromToEvent,
            BlockSpreadEvent,
            BlockPistonEvent,
            InventoryClickEvent,
            InventoryCloseEvent,
            ServerLoadEvent,
            ServerTickStartEvent,
            ServerTickEndEvent,
            WorldLoadEvent,
            WorldUnloadEvent,
            PluginEnableEvent,
            PluginDisableEvent,
        );
        for e in EVENTS {
            if let Some(h) = e.horizon {
                assert!(known.contains(&h), "{} maps to {h}, which the API doesn't have", e.bukkit);
            }
        }
        // And every API event is reachable from some Bukkit name.
        for k in known {
            assert!(EVENTS.iter().any(|e| e.horizon == Some(k)), "nothing maps to {k}");
        }
    }
}
