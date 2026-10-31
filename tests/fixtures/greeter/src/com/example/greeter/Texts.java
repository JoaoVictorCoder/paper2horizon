package com.example.greeter;

import org.bukkit.ChatColor;

final class Texts {
    private Texts() {}

    static String color(String text) {
        return ChatColor.translateAlternateColorCodes('&', text);
    }
}
