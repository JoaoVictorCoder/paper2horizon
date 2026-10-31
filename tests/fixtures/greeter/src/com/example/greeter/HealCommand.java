package com.example.greeter;

import java.util.Arrays;
import java.util.List;
import org.bukkit.Bukkit;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.command.TabCompleter;
import org.bukkit.command.CommandExecutor;
import org.bukkit.entity.Player;

final class HealCommand implements CommandExecutor, TabCompleter {
    private final Greeter plugin;

    HealCommand(Greeter plugin) {
        this.plugin = plugin;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (!sender.hasPermission("greeter.heal")) {
            sender.sendMessage("No permission.");
            return true;
        }
        Player target = args.length > 0 ? null : (Player) sender;
        for (Player p : Bukkit.getOnlinePlayers()) {
            if (args.length > 0 && p.getName().equalsIgnoreCase(args[0])) target = p;
        }
        if (target == null) return false;
        target.setHealth(target.getMaxHealth());
        target.sendMessage("Healed.");
        return true;
    }

    @Override
    public List<String> onTabComplete(CommandSender sender, Command command, String label, String[] args) {
        return Arrays.asList("all", "me");
    }
}
