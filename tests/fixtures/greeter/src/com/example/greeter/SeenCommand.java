package com.example.greeter;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import org.bukkit.Bukkit;
import org.bukkit.ChatColor;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.command.TabCompleter;
import org.bukkit.entity.Player;

/** State in the plugin's collections, and what may be null. */
final class SeenCommand implements CommandExecutor, TabCompleter {
    private final Greeter plugin;

    SeenCommand(Greeter plugin) {
        this.plugin = plugin;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length != 1) {
            return false;
        }
        Player target = Bukkit.getPlayer(args[0]);
        if (target == null) {
            sender.sendMessage(ChatColor.RED + args[0] + " isn't online.");
            return true;
        }
        Long last = plugin.lastSeen.get(target.getUniqueId());
        if (last != null) {
            long minutes = (System.currentTimeMillis() - last) / 60000L;
            sender.sendMessage(target.getName() + " left " + minutes + " minutes ago.");
        } else {
            sender.sendMessage(target.getName() + " is new here.");
        }
        return true;
    }

    @Override
    public List<String> onTabComplete(CommandSender sender, Command command, String label, String[] args) {
        List<String> names = new ArrayList<>();
        for (Player p : Bukkit.getOnlinePlayers()) {
            if (!plugin.muted.contains(p.getUniqueId())) {
                names.add(p.getName());
            }
        }
        return names;
    }
}
