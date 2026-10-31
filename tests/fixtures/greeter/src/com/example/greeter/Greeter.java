package com.example.greeter;

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
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.plugin.java.JavaPlugin;

public final class Greeter extends JavaPlugin implements Listener, CommandExecutor {
    private int joins;

    @Override
    public void onEnable() {
        saveDefaultConfig();
        getServer().getPluginManager().registerEvents(this, this);
        getServer().getPluginManager().registerEvents(new DamageListener(), this);
        getCommand("heal").setExecutor(new HealCommand(this));
        getCommand("heal").setTabCompleter(new HealCommand(this));
        getCommand("spawn").setExecutor(this);
        Bukkit.getScheduler().runTaskTimer(this, () -> Bukkit.broadcastMessage(ChatColor.YELLOW + "Joins so far: " + joins), 20L, 1200L);
        getLogger().info("Greeter ready");
    }

    @EventHandler
    public void onJoin(PlayerJoinEvent event) {
        joins++;
        String text = getConfig().getString("welcome");
        event.getPlayer().sendMessage(ChatColor.translateAlternateColorCodes('&', text.replace("%player%", event.getPlayer().getName())));
        event.setJoinMessage(null);
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
