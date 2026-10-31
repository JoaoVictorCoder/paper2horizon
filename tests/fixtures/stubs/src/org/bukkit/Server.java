package org.bukkit; public interface Server {
  org.bukkit.plugin.PluginManager getPluginManager(); org.bukkit.scheduler.BukkitScheduler getScheduler();
  int broadcastMessage(String m); java.util.Collection<? extends org.bukkit.entity.Player> getOnlinePlayers();
  World getWorld(String name); }
