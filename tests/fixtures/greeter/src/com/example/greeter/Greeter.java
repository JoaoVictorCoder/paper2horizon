package com.example.greeter;

import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import org.bukkit.Bukkit;
import org.bukkit.ChatColor;
import org.bukkit.Material;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.EventPriority;
import org.bukkit.event.Listener;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.event.player.AsyncPlayerChatEvent;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.event.player.PlayerQuitEvent;
import org.bukkit.plugin.java.JavaPlugin;

public final class Greeter extends JavaPlugin implements Listener, CommandExecutor {
    private static Greeter instance;
    private int joins;
    final Map<UUID, Long> lastSeen = new HashMap<>();
    final Set<UUID> muted = new HashSet<>();

    @Override
    public void onEnable() {
        instance = this;
        saveDefaultConfig();
        getServer().getPluginManager().registerEvents(this, this);
        getServer().getPluginManager().registerEvents(new DamageListener(), this);
        getServer().getPluginManager().registerEvents(new AfkListener(), this);
        getCommand("heal").setExecutor(new HealCommand(this));
        getCommand("heal").setTabCompleter(new HealCommand(this));
        getCommand("spawn").setExecutor(this);
        getCommand("greet").setExecutor(new GreetCommand());
        getCommand("seen").setExecutor(new SeenCommand(this));
        getCommand("seen").setTabCompleter(new SeenCommand(this));
        Bukkit.getScheduler().runTaskTimer(this, () -> Bukkit.broadcastMessage(ChatColor.YELLOW + "Joins so far: " + joins), 20L, 1200L);
        getLogger().info("Greeter ready");
    }

    static Greeter getInstance() {
        return instance;
    }

    @EventHandler
    public void onJoin(PlayerJoinEvent event) {
        joins++;
        greet(event.getPlayer());
        event.setJoinMessage(null);
    }

    @EventHandler
    public void onQuit(PlayerQuitEvent event) {
        UUID id = event.getPlayer().getUniqueId();
        lastSeen.put(id, System.currentTimeMillis());
        muted.remove(id);
    }

    @EventHandler
    public void onChat(AsyncPlayerChatEvent event) {
        if (muted.contains(event.getPlayer().getUniqueId())) {
            event.setCancelled(true);
        } else if (event.getMessage().contains("spam")) {
            muted.add(event.getPlayer().getUniqueId());
        }
    }

    /** A helper of the plugin's own, calling a static one of another class. */
    private void greet(Player player) {
        String text = getConfig().getString("welcome");
        player.sendMessage(Texts.color(text.replace("%player%", player.getName())));
    }

    @EventHandler(priority = EventPriority.HIGH, ignoreCancelled = true)
    public void onBreak(BlockBreakEvent event) {
        if (event.getBlock().getType() == Material.BEDROCK && !event.getPlayer().hasPermission("greeter.bedrock")) {
            event.setCancelled(true);
            event.getPlayer().sendMessage(ChatColor.RED + "You can't break bedrock.");
        }
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (!(sender instanceof Player)) {
            sender.sendMessage("Players only.");
            return true;
        }
        Player player = (Player) sender;
        player.teleport(player.getWorld().getSpawnLocation());
        player.sendTitle("Spawn", "", 10, 40, 10);
        return true;
    }
}
