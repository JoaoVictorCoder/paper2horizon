package com.example.greeter;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import org.bukkit.Location;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerMoveEvent;

/** State in a listener's own fields and a static one, reaching the plugin through its singleton. */
final class AfkListener implements Listener {
    static int mutedMoves;
    private final Map<UUID, Long> lastMove = new HashMap<>();
    private int moves = 0;
    private Location lastSpot;

    @EventHandler
    public void onMove(PlayerMoveEvent event) {
        UUID id = event.getPlayer().getUniqueId();
        lastMove.put(id, System.currentTimeMillis());
        moves++;
        if (Greeter.getInstance().muted.contains(id)) {
            mutedMoves++;
        }
        if (lastSpot != null && lastSpot.distance(event.getPlayer().getLocation()) > 100) {
            event.getPlayer().sendMessage("Far from the last spot.");
        }
        lastSpot = event.getPlayer().getLocation();
    }
}
