package org.bukkit.plugin.java;
public abstract class JavaPlugin implements org.bukkit.plugin.Plugin {
  public void onEnable() {} public void onDisable() {} public void onLoad() {}
  public org.bukkit.Server getServer() { return null; }
  public org.bukkit.command.PluginCommand getCommand(String name) { return null; }
  public org.bukkit.configuration.file.FileConfiguration getConfig() { return null; }
  public void saveDefaultConfig() {} public void saveConfig() {} public void reloadConfig() {}
  public java.util.logging.Logger getLogger() { return null; }
  public java.io.File getDataFolder() { return null; }
}
