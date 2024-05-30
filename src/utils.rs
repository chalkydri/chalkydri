//!
//! Small utility functions
//!

/// Generate the team IP
///
/// Let's say you're on team number 12345 (just like all of my passwords).
/// Here's how you'd do that:
///
/// ```text
/// 1   2   3   4   5
/// |___|___|   |___|
///        \     /
///     10.123.45.2
/// ```
///
/// Reference:
/// <https://docs.wpilib.org/en/stable/docs/networking/networking-introduction/ip-configurations.html#te-am-ip-notation>
pub fn gen_team_ip(team_number: u16) -> Option<[u8; 4]> {
    if team_number > 25_599 {
        None
    } else {
        Some([10, (team_number / 100) as u8, (team_number % 100) as u8, 2])
    }
}

