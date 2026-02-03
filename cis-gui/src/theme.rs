//! # GUI Theme Colors
//!
//! Color definitions for node trust states and UI elements.

use eframe::egui::Color32;

// ==================== Trust State Colors ====================

/// Verified node - Local (orange tint)
pub const VERIFIED_LOCAL_BG: Color32 = Color32::from_rgb(212, 165, 116);      // #D4A574
pub const VERIFIED_LOCAL_TEXT: Color32 = Color32::from_rgb(93, 78, 55);       // #5D4E37
pub const VERIFIED_LOCAL_ACTIVE: Color32 = Color32::from_rgb(232, 212, 196);  // #E8D4C4

/// Verified node - Cloud (blue tint)
pub const VERIFIED_CLOUD_BG: Color32 = Color32::from_rgb(123, 167, 212);      // #7BA7D4
pub const VERIFIED_CLOUD_TEXT: Color32 = Color32::from_rgb(55, 74, 93);       // #374A5D
pub const VERIFIED_CLOUD_ACTIVE: Color32 = Color32::from_rgb(196, 216, 232);  // #C4D8E8

/// Pending verification (yellow)
pub const PENDING_BG: Color32 = Color32::from_rgb(241, 196, 15);              // #F1C40F
pub const PENDING_TEXT: Color32 = Color32::from_rgb(40, 40, 40);              // #282828

/// Unknown/Not verified (gray)
pub const UNKNOWN_BG: Color32 = Color32::from_rgb(128, 128, 128);             // #808080
pub const UNKNOWN_TEXT: Color32 = Color32::from_rgb(255, 255, 255);           // #FFFFFF

/// Blocked (red)
pub const BLOCKED_BG: Color32 = Color32::from_rgb(231, 76, 60);               // #E74C3C
pub const BLOCKED_TEXT: Color32 = Color32::from_rgb(255, 255, 255);           // #FFFFFF

// ==================== UI Colors ====================

/// Terminal background
pub const TERMINAL_BG: Color32 = Color32::from_rgb(30, 30, 30);               // #1E1E1E
pub const TERMINAL_FG: Color32 = Color32::from_rgb(212, 212, 212);            // #D4D4D4
pub const TERMINAL_GREEN: Color32 = Color32::from_rgb(152, 195, 121);         // #98C379
pub const TERMINAL_YELLOW: Color32 = Color32::from_rgb(229, 192, 123);        // #E5C07B
pub const TERMINAL_RED: Color32 = Color32::from_rgb(224, 108, 117);           // #E06C75

/// Main background
pub const MAIN_BG: Color32 = Color32::from_rgb(40, 40, 40);                   // #282828
pub const PANEL_BG: Color32 = Color32::from_rgb(50, 50, 50);                  // #323232

/// Text colors
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 220);           // #DCDCDC
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 150, 150);         // #969696

/// Status indicators
pub const STATUS_ONLINE: Color32 = Color32::from_rgb(46, 204, 113);           // #2ECC71
pub const STATUS_OFFLINE: Color32 = Color32::from_rgb(149, 165, 166);         // #95A5A6
pub const STATUS_WARNING: Color32 = Color32::from_rgb(241, 196, 15);          // #F1C40F
pub const STATUS_ERROR: Color32 = Color32::from_rgb(231, 76, 60);             // #E74C3C

// ==================== Helper Functions ====================

/// Get colors for a trust state
pub fn trust_state_colors(is_verified: bool, is_local: bool) -> (Color32, Color32) {
    if is_verified {
        if is_local {
            (VERIFIED_LOCAL_BG, VERIFIED_LOCAL_TEXT)
        } else {
            (VERIFIED_CLOUD_BG, VERIFIED_CLOUD_TEXT)
        }
    } else {
        (UNKNOWN_BG, UNKNOWN_TEXT)
    }
}

/// Get status dot color
pub fn status_dot_color(online: bool) -> Color32 {
    if online {
        STATUS_ONLINE
    } else {
        STATUS_OFFLINE
    }
}
