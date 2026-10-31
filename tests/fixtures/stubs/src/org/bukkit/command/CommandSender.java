package org.bukkit.command; public interface CommandSender { void sendMessage(String m); boolean hasPermission(String p); String getName(); boolean isOp(); }
