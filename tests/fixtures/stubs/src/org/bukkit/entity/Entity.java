package org.bukkit.entity; public interface Entity { org.bukkit.Location getLocation(); boolean teleport(org.bukkit.Location l); org.bukkit.World getWorld(); java.util.UUID getUniqueId(); }
