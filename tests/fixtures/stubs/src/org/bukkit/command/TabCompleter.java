package org.bukkit.command; public interface TabCompleter { java.util.List<String> onTabComplete(CommandSender s, Command c, String label, String[] args); }
