package com.example.greeter;

import org.bukkit.Bukkit;
import org.bukkit.ChatColor;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;

/** A string switch, a loop over the players, an early return and `false` for the usage. */
public class GreetCommand implements CommandExecutor {
    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length == 0) {
            return false;
        }
        switch (args[0].toLowerCase()) {
            case "all":
            case "everyone":
                for (Player p : Bukkit.getOnlinePlayers()) {
                    p.sendMessage(ChatColor.GREEN + "Hello from " + sender.getName() + "!");
                }
                break;
            case "me":
                sender.sendMessage("Hello, " + sender.getName() + ".");
                break;
            default:
                sender.sendMessage(ChatColor.RED + "Unknown: " + args[0]);
                return true;
        }
        return true;
    }
}
